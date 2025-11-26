// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! High-level DSA engine API.

use crate::device::discover_devices;
use crate::error::DsaError;
use crate::wq::WorkQueue;
use std::path::Path;

/// High-level DSA engine providing safe access to DSA operations.
///
/// `DsaEngine` wraps a work queue and provides convenient methods for
/// common DSA operations like CRC32, memory copy, fill, and compare.
///
/// # Example
///
/// ```rust,no_run
/// use dsa_rust::{DsaEngine, DsaError};
///
/// fn main() -> Result<(), DsaError> {
///     let engine = DsaEngine::open_first()?;
///
///     let data = b"Hello, DSA!";
///     let crc = engine.crc32(data)?;
///     println!("CRC32: {:#010x}", crc);
///
///     Ok(())
/// }
/// ```
pub struct DsaEngine {
    wq: WorkQueue,
}

impl DsaEngine {
    /// Open the first available DSA work queue.
    ///
    /// This discovers all DSA devices on the system and opens the first
    /// enabled work queue found.
    ///
    /// # Platform Behavior
    ///
    /// - **Linux**: Opens a hardware-accelerated work queue via IDXD driver
    /// - **Windows**: Opens a software-emulated work queue (hardware DSA
    ///   access is not available on Windows)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No DSA devices are found (Linux only)
    /// - No enabled work queues are available
    /// - Failed to open the work queue
    #[cfg(target_os = "linux")]
    pub fn open_first() -> Result<Self, DsaError> {
        let devices = discover_devices()?;
        let device = devices.into_iter().next().ok_or(DsaError::NoDeviceFound)?;
        let wq = device.open_first_wq()?;
        Ok(Self { wq })
    }

    /// Open a software-emulated DSA engine on Windows.
    ///
    /// On Windows, hardware DSA access is not available through userspace APIs.
    /// This creates a software-emulated work queue that provides the same API
    /// but uses optimized software implementations (e.g., crc32fast for CRC32).
    #[cfg(target_os = "windows")]
    pub fn open_first() -> Result<Self, DsaError> {
        // Try to discover hardware first (for informational purposes)
        match discover_devices() {
            Ok(devices) if !devices.is_empty() => {
                log::info!(
                    "Found {} DSA device(s), using software fallback (Windows)",
                    devices.len()
                );
            }
            _ => {
                log::info!("No DSA hardware detected, using software fallback");
            }
        }

        // Always use software work queue on Windows
        let wq = WorkQueue::open(std::path::Path::new(""))?;
        Ok(Self { wq })
    }

    /// Open a software-emulated DSA engine (platform-independent fallback).
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    pub fn open_first() -> Result<Self, DsaError> {
        Err(DsaError::PlatformNotSupported)
    }

    /// Open a specific work queue by path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the work queue device (e.g., `/dev/dsa/wq0.0`)
    ///
    /// # Errors
    ///
    /// Returns an error if the work queue cannot be opened.
    pub fn open(path: &Path) -> Result<Self, DsaError> {
        let wq = WorkQueue::open(path)?;
        Ok(Self { wq })
    }

    /// Get a reference to the underlying work queue.
    pub fn work_queue(&self) -> &WorkQueue {
        &self.wq
    }

    /// Get a mutable reference to the underlying work queue.
    pub fn work_queue_mut(&mut self) -> &mut WorkQueue {
        &mut self.wq
    }

    /// Compute CRC32 checksum of the given data using DSA hardware.
    ///
    /// This offloads CRC32 computation to the DSA accelerator, freeing
    /// CPU cycles for other work. Most efficient for buffers >= 4KB.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to compute checksum over
    ///
    /// # Returns
    ///
    /// The CRC32 checksum value.
    pub fn crc32(&self, data: &[u8]) -> Result<u32, DsaError> {
        self.crc32_with_seed(data, 0)
    }

    /// Compute CRC32 checksum with an initial seed value.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to compute checksum over
    /// * `seed` - Initial CRC value (for chaining CRC computations)
    ///
    /// # Returns
    ///
    /// The CRC32 checksum value.
    pub fn crc32_with_seed(&self, data: &[u8], seed: u32) -> Result<u32, DsaError> {
        self.wq.crc32(data, seed)
    }

    /// Copy memory from source to destination using DSA hardware.
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination buffer (must be at least as large as `src`)
    /// * `src` - Source buffer
    ///
    /// # Errors
    ///
    /// Returns an error if `dst` is smaller than `src` or the operation fails.
    pub fn memcpy(&self, dst: &mut [u8], src: &[u8]) -> Result<(), DsaError> {
        self.wq.memcpy(dst, src)
    }

    /// Fill memory with a 64-bit pattern using DSA hardware.
    ///
    /// The pattern is repeated to fill the entire destination buffer.
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination buffer to fill
    /// * `pattern` - 64-bit pattern to fill with
    pub fn memset(&self, dst: &mut [u8], pattern: u64) -> Result<(), DsaError> {
        self.wq.memset(dst, pattern)
    }

    /// Compare two memory regions using DSA hardware.
    ///
    /// # Arguments
    ///
    /// * `a` - First buffer
    /// * `b` - Second buffer (must be same length as `a`)
    ///
    /// # Returns
    ///
    /// `Ok(true)` if regions are equal, `Ok(false)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes don't match or the operation fails.
    pub fn memcmp(&self, a: &[u8], b: &[u8]) -> Result<bool, DsaError> {
        self.wq.memcmp(a, b)
    }

    /// Execute a no-op operation (for testing/benchmarking).
    ///
    /// This submits a descriptor that does nothing, useful for measuring
    /// submission overhead.
    pub fn noop(&self) -> Result<(), DsaError> {
        self.wq.noop()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_engine_requires_hardware() {
        // DsaEngine tests require actual DSA hardware
        // These are integration tests that should be skipped on systems without DSA
    }
}
