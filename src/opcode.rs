// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! DSA operation codes.
//!
//! These opcodes are defined in the Intel DSA Architecture Specification
//! and match the Linux kernel's `include/uapi/linux/idxd.h` definitions.

/// DSA operation codes.
///
/// Each operation has a unique 8-bit opcode that is placed in the
/// descriptor's opcode field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DsaOpcode {
    /// No operation - used for testing/synchronization.
    Noop = 0x00,

    /// Batch operation - submit multiple descriptors at once.
    Batch = 0x01,

    /// Drain - wait for all previous operations to complete.
    Drain = 0x03,

    /// Memory move (copy) operation.
    MemMove = 0x04,

    /// Memory fill operation.
    MemFill = 0x05,

    /// Memory compare operation.
    Compare = 0x06,

    /// Compare with immediate value.
    CompareImm = 0x07,

    /// Create delta record between two buffers.
    CreateDelta = 0x08,

    /// Apply delta record to a buffer.
    ApplyDelta = 0x09,

    /// Dual-cast memory copy (copy to two destinations).
    Dualcast = 0x0A,

    /// Translation fetch (prefetch with address translation).
    TranslFetch = 0x0D,

    /// CRC32 generation.
    CrcGen = 0x10,

    /// Copy with CRC32 generation.
    CopyCrc = 0x12,

    /// DIF (Data Integrity Field) check.
    DifCheck = 0x13,

    /// DIF insert.
    DifInsert = 0x14,

    /// DIF strip.
    DifStrip = 0x15,

    /// DIF update.
    DifUpdate = 0x16,

    /// DIX (Data Integrity Extension) generate.
    DixGen = 0x17,

    /// Cache flush.
    CacheFlush = 0x20,
}

impl DsaOpcode {
    /// Returns the opcode as a u8 value.
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns a human-readable name for the opcode.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Noop => "NOOP",
            Self::Batch => "BATCH",
            Self::Drain => "DRAIN",
            Self::MemMove => "MEMMOVE",
            Self::MemFill => "MEMFILL",
            Self::Compare => "COMPARE",
            Self::CompareImm => "COMPARE_IMM",
            Self::CreateDelta => "CREATE_DELTA",
            Self::ApplyDelta => "APPLY_DELTA",
            Self::Dualcast => "DUALCAST",
            Self::TranslFetch => "TRANSL_FETCH",
            Self::CrcGen => "CRC_GEN",
            Self::CopyCrc => "COPY_CRC",
            Self::DifCheck => "DIF_CHECK",
            Self::DifInsert => "DIF_INSERT",
            Self::DifStrip => "DIF_STRIP",
            Self::DifUpdate => "DIF_UPDATE",
            Self::DixGen => "DIX_GEN",
            Self::CacheFlush => "CACHE_FLUSH",
        }
    }
}

impl std::fmt::Display for DsaOpcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:#04x})", self.name(), self.as_u8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_values() {
        assert_eq!(DsaOpcode::Noop.as_u8(), 0x00);
        assert_eq!(DsaOpcode::Batch.as_u8(), 0x01);
        assert_eq!(DsaOpcode::Drain.as_u8(), 0x03);
        assert_eq!(DsaOpcode::MemMove.as_u8(), 0x04);
        assert_eq!(DsaOpcode::MemFill.as_u8(), 0x05);
        assert_eq!(DsaOpcode::Compare.as_u8(), 0x06);
        assert_eq!(DsaOpcode::CrcGen.as_u8(), 0x10);
        assert_eq!(DsaOpcode::CopyCrc.as_u8(), 0x12);
        assert_eq!(DsaOpcode::CacheFlush.as_u8(), 0x20);
    }

    #[test]
    fn test_opcode_display() {
        assert_eq!(format!("{}", DsaOpcode::CrcGen), "CRC_GEN (0x10)");
        assert_eq!(format!("{}", DsaOpcode::MemMove), "MEMMOVE (0x04)");
    }
}
