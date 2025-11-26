// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! Low-level descriptor submission using MOVDIR64B and ENQCMD instructions.
//!
//! # Instruction Overview
//!
//! - **MOVDIR64B**: Posted 64-byte write for Dedicated Work Queues (DWQ).
//!   Cannot report device errors, but has lower overhead.
//!
//! - **ENQCMD**: Non-posted 64-byte write for Shared Work Queues (SWQ).
//!   Reports queue full/device busy via EFLAGS.ZF, requires PASID.
//!
//! # Safety
//!
//! These functions are unsafe because:
//! - The portal address must be valid and properly mapped
//! - The descriptor must remain valid during submission
//! - The completion record must remain valid until the operation completes

use crate::descriptor::DsaHwDesc;

/// Submit a descriptor to a Dedicated Work Queue using MOVDIR64B.
///
/// # Safety
///
/// - `portal` must be a valid memory-mapped DSA portal address (64-byte aligned)
/// - `desc` must be a valid, properly initialized 64-byte descriptor
/// - The completion record referenced by `desc` must remain valid
///
/// # Notes
///
/// MOVDIR64B is a posted write - it does not wait for the device to accept
/// the descriptor. The caller must ensure not to exceed the work queue depth.
///
/// # Instruction Details
///
/// `MOVDIR64B r64, m512` reads 64 bytes from the source memory operand and
/// performs a 64-byte direct-store to the destination address in the register.
/// - Register operand (r64): Contains destination address (portal)
/// - Memory operand (m512): Source of 64 bytes (descriptor)
#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn movdir64b(portal: *mut u8, desc: &DsaHwDesc) {
    // MOVDIR64B instruction encoding:
    // 66 0F 38 F8 /r - MOVDIR64B r64, m512
    //
    // ModR/M byte 0x02:
    //   mod = 00 (memory, no displacement)
    //   reg = 000 (RAX - contains destination address)
    //   r/m = 010 (RDX - memory base for source)
    //
    // This matches Linux kernel's implementation in arch/x86/include/asm/special_insns.h
    //
    // Operation:
    // 1. Read 64 bytes from [RDX] (descriptor)
    // 2. Write 64 bytes to address in RAX (portal) using direct-store
    core::arch::asm!(
        ".byte 0x66, 0x0f, 0x38, 0xf8, 0x02",
        in("rax") portal,
        in("rdx") desc as *const DsaHwDesc,
        options(nostack, preserves_flags)
    );
}

/// Submit a descriptor to a Shared Work Queue using ENQCMD.
///
/// Returns `true` if the descriptor was successfully enqueued,
/// `false` if the queue was full (retry later).
///
/// # Safety
///
/// - `portal` must be a valid memory-mapped DSA portal address (64-byte aligned)
/// - `desc` must be a valid, properly initialized 64-byte descriptor
/// - The completion record referenced by `desc` must remain valid
/// - The process must have a valid PASID bound via iommu_sva_bind_device
///
/// # Notes
///
/// ENQCMD is a non-posted write - it waits for the device to respond.
/// The instruction sets EFLAGS.ZF=0 on success, ZF=1 on failure (queue full/retry).
///
/// # Instruction Details
///
/// `ENQCMD r64, m512` reads 64 bytes from the source memory operand,
/// auto-fills the PASID field from IA32_PASID MSR, and submits to the
/// device at the destination address.
/// - Register operand (r64): Contains destination address (portal)
/// - Memory operand (m512): Source of 64 bytes (descriptor)
#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn enqcmd(portal: *mut u8, desc: &DsaHwDesc) -> bool {
    // ENQCMD instruction encoding:
    // F3 0F 38 F8 /r - ENQCMD r64, m512
    //
    // ModR/M byte 0x02:
    //   mod = 00 (memory, no displacement)
    //   reg = 000 (RAX - contains destination address)
    //   r/m = 010 (RDX - memory base for source)
    //
    // This matches Linux kernel's implementation in arch/x86/include/asm/special_insns.h
    //
    // Operation:
    // 1. Read 64 bytes from [RDX] (descriptor)
    // 2. Auto-fill PASID from IA32_PASID MSR
    // 3. Submit to device at address in RAX (portal)
    // 4. Set ZF=0 on success, ZF=1 on failure (queue full)
    //
    // Note: Linux kernel uses ZF=0 as success, ZF=1 as failure (opposite of intuition)
    let failed: u8;
    core::arch::asm!(
        ".byte 0xf3, 0x0f, 0x38, 0xf8, 0x02",
        "setz {failed}",
        in("rax") portal,
        in("rdx") desc as *const DsaHwDesc,
        failed = out(reg_byte) failed,
        options(nostack)
    );
    failed == 0 // ZF=0 means success
}

/// Submit a descriptor with automatic retry on queue full.
///
/// # Safety
///
/// Same requirements as [`enqcmd`].
///
/// # Parameters
///
/// - `portal`: DSA portal address
/// - `desc`: Descriptor to submit
/// - `max_retries`: Maximum number of retries before giving up
///
/// # Returns
///
/// `true` if successfully submitted, `false` if max retries exceeded.
#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn enqcmd_retry(portal: *mut u8, desc: &DsaHwDesc, max_retries: u32) -> bool {
    for _ in 0..max_retries {
        if enqcmd(portal, desc) {
            return true;
        }
        // Brief pause to allow queue to drain
        core::hint::spin_loop();
    }
    false
}

/// Result of a submission attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitResult {
    /// Descriptor was successfully submitted.
    Success,
    /// Work queue is full, try again later.
    QueueFull,
}

/// Work queue submission mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitMode {
    /// Use MOVDIR64B for Dedicated Work Queues.
    Dedicated,
    /// Use ENQCMD for Shared Work Queues.
    Shared,
}

/// Submit a descriptor using the appropriate instruction for the work queue type.
///
/// # Safety
///
/// - `portal` must be a valid memory-mapped DSA portal address
/// - `desc` must be a valid, properly initialized descriptor
/// - The completion record referenced by `desc` must remain valid
/// - For Shared mode, the process must have a valid PASID
#[inline]
#[cfg(target_arch = "x86_64")]
pub unsafe fn submit(portal: *mut u8, desc: &DsaHwDesc, mode: SubmitMode) -> SubmitResult {
    match mode {
        SubmitMode::Dedicated => {
            movdir64b(portal, desc);
            SubmitResult::Success
        }
        SubmitMode::Shared => {
            if enqcmd(portal, desc) {
                SubmitResult::Success
            } else {
                SubmitResult::QueueFull
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_result() {
        assert_eq!(SubmitResult::Success, SubmitResult::Success);
        assert_ne!(SubmitResult::Success, SubmitResult::QueueFull);
    }

    #[test]
    fn test_submit_mode() {
        assert_eq!(SubmitMode::Dedicated, SubmitMode::Dedicated);
        assert_ne!(SubmitMode::Dedicated, SubmitMode::Shared);
    }

    // Note: Actual submission tests require real DSA hardware
    // and are skipped in unit tests.
}
