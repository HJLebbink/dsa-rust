// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! Error types for DSA operations.

use thiserror::Error;

/// Errors that can occur during DSA operations.
#[derive(Debug, Error)]
pub enum DsaError {
    /// No DSA device found on the system.
    #[error("no DSA device found")]
    NoDeviceFound,

    /// No work queue available on the device.
    #[error("no work queue available")]
    NoWorkQueue,

    /// Work queue is full (ENQCMD returned busy).
    #[error("work queue full")]
    QueueFull,

    /// DSA operation failed with hardware error.
    #[error("DSA operation failed: status={status:#04x}, result={result:#04x}")]
    OperationFailed { status: u8, result: u8 },

    /// Page fault during DSA operation.
    #[error("page fault at address {fault_addr:#018x}, completed {bytes_completed} bytes")]
    PageFault {
        fault_addr: u64,
        bytes_completed: u32,
    },

    /// Invalid argument provided.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Buffer size mismatch.
    #[error("buffer size mismatch: expected {expected}, got {actual}")]
    BufferSizeMismatch { expected: usize, actual: usize },

    /// I/O error from system calls.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Platform not supported.
    #[error("platform not supported: DSA requires Linux with IDXD driver")]
    PlatformNotSupported,

    /// Device not enabled or configured.
    #[error("DSA device not enabled or not configured")]
    DeviceNotEnabled,

    /// Permission denied accessing DSA device.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Memory mapping failed.
    #[error("mmap failed: {0}")]
    MmapFailed(String),
}

/// Result type alias for DSA operations.
pub type DsaResult<T> = Result<T, DsaError>;
