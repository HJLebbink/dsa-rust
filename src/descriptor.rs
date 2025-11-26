// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! DSA hardware descriptor and completion record structures.
//!
//! These structures match the hardware layout defined in the Intel DSA
//! Architecture Specification and Linux kernel's `include/uapi/linux/idxd.h`.

use crate::opcode::DsaOpcode;
use bitflags::bitflags;

bitflags! {
    /// Descriptor flags (bits 0-23 of the flags/opcode field).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DescriptorFlags: u32 {
        /// Request completion record.
        const REQUEST_COMPLETION = 1 << 0;
        /// Request completion interrupt.
        const COMPLETION_INTERRUPT = 1 << 1;
        /// Fence - wait for previous descriptors.
        const FENCE = 1 << 2;
        /// Block on fault - don't return partial completion on page fault.
        const BLOCK_ON_FAULT = 1 << 3;
        /// Read source 2 as aecs (for crypto operations).
        const SRC2_AECS = 1 << 4;
        /// Destination is steering tag.
        const DEST_STEERING_TAG = 1 << 5;
        /// Completion record address is valid.
        const CR_ADDR_VALID = 1 << 6;
        /// Request status writeback.
        const STATUS_WRITEBACK = 1 << 7;
        /// Destination readback.
        const DEST_READBACK = 1 << 8;
        /// Cache control - don't allocate destination in cache.
        const CACHE_CTRL = 1 << 9;
    }
}

/// 64-byte DSA hardware descriptor.
///
/// This structure is submitted to the DSA hardware via MOVDIR64B or ENQCMD
/// instructions. It must be 64-byte aligned.
///
/// # Layout
///
/// The descriptor layout varies slightly depending on the operation,
/// but the first 32 bytes are common to all operations.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct DsaHwDesc {
    /// PASID (Process Address Space ID) and privilege level.
    /// Bits [19:0] = PASID, Bits [30:20] = reserved, Bit [31] = privileged.
    pub pasid: u32,

    /// Flags (bits [23:0]) and opcode (bits [31:24]).
    pub flags_opcode: u32,

    /// Address of completion record (must be 32-byte aligned).
    pub completion_addr: u64,

    /// Source address (operation-dependent meaning).
    /// - MemMove/CrcGen: source data address
    /// - MemFill: 64-bit pattern (lower bits)
    /// - Batch: descriptor list address
    pub src_addr: u64,

    /// Destination address (operation-dependent meaning).
    /// - MemMove: destination data address
    /// - Compare: second source address
    /// - MemFill: destination address
    pub dst_addr: u64,

    /// Transfer size in bytes.
    pub xfer_size: u32,

    /// Interrupt handle for completion interrupts.
    pub int_handle: u16,

    /// Reserved field.
    reserved1: u16,

    // Operation-specific fields (bytes 40-63)
    /// Source 2 address or delta record address.
    pub src2_addr: u64,

    /// Maximum delta record size or CRC seed.
    /// For CRC operations, bits [31:0] contain the seed value.
    pub crc_seed_or_delta_size: u64,

    /// Reserved/operation-specific field.
    reserved2: u64,
}

impl DsaHwDesc {
    /// Create a new zeroed descriptor.
    #[inline]
    pub const fn new() -> Self {
        Self {
            pasid: 0,
            flags_opcode: 0,
            completion_addr: 0,
            src_addr: 0,
            dst_addr: 0,
            xfer_size: 0,
            int_handle: 0,
            reserved1: 0,
            src2_addr: 0,
            crc_seed_or_delta_size: 0,
            reserved2: 0,
        }
    }

    /// Set the opcode for this descriptor.
    #[inline]
    pub fn set_opcode(&mut self, opcode: DsaOpcode) {
        // Opcode is in bits [31:24]
        self.flags_opcode = (self.flags_opcode & 0x00FFFFFF) | ((opcode.as_u8() as u32) << 24);
    }

    /// Get the opcode from this descriptor.
    #[inline]
    pub fn opcode(&self) -> u8 {
        (self.flags_opcode >> 24) as u8
    }

    /// Set descriptor flags.
    #[inline]
    pub fn set_flags(&mut self, flags: DescriptorFlags) {
        // Flags are in bits [23:0]
        self.flags_opcode = (self.flags_opcode & 0xFF000000) | (flags.bits() & 0x00FFFFFF);
    }

    /// Add descriptor flags (OR with existing).
    #[inline]
    pub fn add_flags(&mut self, flags: DescriptorFlags) {
        self.flags_opcode |= flags.bits() & 0x00FFFFFF;
    }

    /// Set the completion record address.
    #[inline]
    pub fn set_completion(&mut self, record: &mut DsaCompletionRecord) {
        self.completion_addr = record as *mut _ as u64;
        self.add_flags(DescriptorFlags::REQUEST_COMPLETION);
    }

    /// Create a CRC generation descriptor.
    pub fn crc_gen(
        src: *const u8,
        len: usize,
        seed: u32,
        completion: &mut DsaCompletionRecord,
    ) -> Self {
        let mut desc = Self::new();
        desc.set_opcode(DsaOpcode::CrcGen);
        desc.src_addr = src as u64;
        desc.xfer_size = len as u32;
        desc.crc_seed_or_delta_size = seed as u64;
        desc.set_completion(completion);
        desc
    }

    /// Create a memory move (copy) descriptor.
    pub fn mem_move(
        dst: *mut u8,
        src: *const u8,
        len: usize,
        completion: &mut DsaCompletionRecord,
    ) -> Self {
        let mut desc = Self::new();
        desc.set_opcode(DsaOpcode::MemMove);
        desc.src_addr = src as u64;
        desc.dst_addr = dst as u64;
        desc.xfer_size = len as u32;
        desc.set_completion(completion);
        desc
    }

    /// Create a memory fill descriptor.
    pub fn mem_fill(
        dst: *mut u8,
        len: usize,
        pattern: u64,
        completion: &mut DsaCompletionRecord,
    ) -> Self {
        let mut desc = Self::new();
        desc.set_opcode(DsaOpcode::MemFill);
        desc.src_addr = pattern; // Pattern goes in src_addr for MemFill
        desc.dst_addr = dst as u64;
        desc.xfer_size = len as u32;
        desc.set_completion(completion);
        desc
    }

    /// Create a memory compare descriptor.
    pub fn compare(
        src1: *const u8,
        src2: *const u8,
        len: usize,
        completion: &mut DsaCompletionRecord,
    ) -> Self {
        let mut desc = Self::new();
        desc.set_opcode(DsaOpcode::Compare);
        desc.src_addr = src1 as u64;
        desc.dst_addr = src2 as u64; // Second source goes in dst_addr for Compare
        desc.xfer_size = len as u32;
        desc.set_completion(completion);
        desc
    }

    /// Create a no-op descriptor (useful for testing/synchronization).
    pub fn noop(completion: &mut DsaCompletionRecord) -> Self {
        let mut desc = Self::new();
        desc.set_opcode(DsaOpcode::Noop);
        desc.set_completion(completion);
        desc
    }
}

impl Default for DsaHwDesc {
    fn default() -> Self {
        Self::new()
    }
}

/// 64-byte DSA completion record.
///
/// The DSA hardware writes to this structure when an operation completes.
/// It must be 32-byte aligned and the `status` field should be read with
/// volatile semantics.
///
/// # Layout (per Intel DSA Architecture Specification)
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 1 | status |
/// | 1 | 1 | result |
/// | 2 | 1 | fault_info |
/// | 3 | 1 | reserved |
/// | 4 | 4 | bytes_completed |
/// | 8 | 8 | fault_addr |
/// | 16 | 8 | result_value (CRC, etc.) |
/// | 24 | 8 | result_value2 (extended results) |
/// | 32 | 32 | operation-specific (DIF, etc.) |
///
/// The structure is 64 bytes to match the Linux kernel's `dsa_completion_record`
/// and support all DSA operations including DIF.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
pub struct DsaCompletionRecord {
    /// Completion status (non-zero when complete).
    /// Use volatile read via `is_complete()` or `get_status()`.
    pub status: u8,

    /// Result code (operation-specific).
    /// - For Compare: 0 = equal, 1 = not equal
    /// - For other ops: error details
    pub result: u8,

    /// Fault information flags.
    pub fault_info: u8,

    /// Reserved.
    reserved1: u8,

    /// Number of bytes completed (for partial completions on page fault).
    pub bytes_completed: u32,

    /// Fault address (if page fault occurred).
    pub fault_addr: u64,

    /// Primary result value (operation-dependent).
    /// - CRC operations: CRC32 value in bits [31:0]
    /// - Compare: first differing offset
    pub result_value: u64,

    /// Secondary result value (extended operations).
    /// - CRC64: upper bits of CRC
    /// - DIF operations: additional status
    pub result_value2: u64,

    /// Operation-specific extended results.
    /// Used by DIF check/insert/update operations for detailed status.
    /// For basic operations (CRC, memcpy, etc.), this field is unused.
    reserved_op_specific: [u8; 32],
}

impl DsaCompletionRecord {
    /// Create a new zeroed completion record.
    #[inline]
    pub const fn new() -> Self {
        Self {
            status: 0,
            result: 0,
            fault_info: 0,
            reserved1: 0,
            bytes_completed: 0,
            fault_addr: 0,
            result_value: 0,
            result_value2: 0,
            reserved_op_specific: [0; 32],
        }
    }

    /// Reset the completion record for reuse.
    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Check if the operation has completed (volatile read).
    #[inline]
    pub fn is_complete(&self) -> bool {
        // Use volatile read to prevent compiler from caching the value
        unsafe { std::ptr::read_volatile(&self.status) != 0 }
    }

    /// Get the completion status (volatile read).
    #[inline]
    pub fn get_status(&self) -> CompletionStatus {
        let status = unsafe { std::ptr::read_volatile(&self.status) };
        CompletionStatus::from(status)
    }

    /// Get the CRC32 result value (for CRC operations).
    #[inline]
    pub fn crc32_result(&self) -> u32 {
        self.result_value as u32
    }

    /// Get the comparison result (for Compare operations).
    /// Returns true if buffers are equal.
    #[inline]
    pub fn compare_result(&self) -> bool {
        self.result == 0
    }
}

impl Default for DsaCompletionRecord {
    fn default() -> Self {
        Self::new()
    }
}

/// Completion status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStatus {
    /// Operation not yet complete.
    Pending,
    /// Operation completed successfully.
    Success,
    /// Page fault occurred.
    PageFault,
    /// Invalid flags in descriptor.
    InvalidFlags,
    /// Unsupported operation.
    UnsupportedOp,
    /// Invalid transfer size.
    InvalidSize,
    /// Invalid completion record address.
    InvalidCompletionAddr,
    /// Hardware error.
    HardwareError,
    /// Unknown status code.
    Unknown(u8),
}

impl From<u8> for CompletionStatus {
    fn from(status: u8) -> Self {
        match status {
            0x00 => Self::Pending,
            0x01 => Self::Success,
            0x03 => Self::PageFault,
            0x10 => Self::InvalidFlags,
            0x11 => Self::UnsupportedOp,
            0x13 => Self::InvalidSize,
            0x19 => Self::InvalidCompletionAddr,
            0x1F => Self::HardwareError,
            _ => Self::Unknown(status),
        }
    }
}

impl CompletionStatus {
    /// Returns true if this status indicates success.
    #[inline]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Returns true if this status indicates the operation is still pending.
    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns true if this status indicates an error.
    #[inline]
    pub fn is_error(&self) -> bool {
        !matches!(self, Self::Pending | Self::Success)
    }
}

// Compile-time size and alignment checks per Intel DSA Architecture Specification
const _: () = assert!(std::mem::size_of::<DsaHwDesc>() == 64);
const _: () = assert!(std::mem::align_of::<DsaHwDesc>() == 64);
const _: () = assert!(std::mem::size_of::<DsaCompletionRecord>() == 64);
const _: () = assert!(std::mem::align_of::<DsaCompletionRecord>() == 32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptor_size_and_alignment() {
        assert_eq!(std::mem::size_of::<DsaHwDesc>(), 64);
        assert_eq!(std::mem::align_of::<DsaHwDesc>(), 64);
    }

    #[test]
    fn test_completion_record_size_and_alignment() {
        assert_eq!(std::mem::size_of::<DsaCompletionRecord>(), 64);
        assert_eq!(std::mem::align_of::<DsaCompletionRecord>(), 32);
    }

    #[test]
    fn test_set_opcode() {
        let mut desc = DsaHwDesc::new();
        desc.set_opcode(DsaOpcode::CrcGen);
        assert_eq!(desc.opcode(), 0x10);

        desc.set_opcode(DsaOpcode::MemMove);
        assert_eq!(desc.opcode(), 0x04);
    }

    #[test]
    fn test_set_flags() {
        let mut desc = DsaHwDesc::new();
        desc.set_flags(DescriptorFlags::REQUEST_COMPLETION | DescriptorFlags::FENCE);

        // Verify flags don't affect opcode
        desc.set_opcode(DsaOpcode::CrcGen);
        assert_eq!(desc.opcode(), 0x10);

        // Verify flags are preserved
        assert!(desc.flags_opcode & DescriptorFlags::REQUEST_COMPLETION.bits() != 0);
        assert!(desc.flags_opcode & DescriptorFlags::FENCE.bits() != 0);
    }

    #[test]
    fn test_completion_status() {
        assert!(CompletionStatus::Success.is_success());
        assert!(CompletionStatus::Pending.is_pending());
        assert!(CompletionStatus::PageFault.is_error());
        assert!(CompletionStatus::InvalidFlags.is_error());
    }

    #[test]
    fn test_completion_record_volatile_read() {
        let mut record = DsaCompletionRecord::new();
        assert!(!record.is_complete());

        // Simulate hardware completion
        record.status = 0x01;
        assert!(record.is_complete());
        assert!(record.get_status().is_success());
    }
}
