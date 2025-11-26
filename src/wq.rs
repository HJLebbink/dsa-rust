// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! Work queue management and operations.
//!
//! A work queue is the interface through which applications submit work to DSA.
//! Each work queue has an associated portal (memory-mapped region) where
//! descriptors are submitted.
//!
//! # Platform Support
//!
//! Currently only Linux is supported. On other platforms, attempting to open
//! a work queue will return `DsaError::PlatformNotSupported`.

use crate::error::DsaError;
use std::path::Path;

#[cfg(target_os = "linux")]
use crate::submit::{enqcmd_retry, movdir64b};
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

/// Portal size for mmap (one page).
#[cfg(target_os = "linux")]
const PORTAL_SIZE: usize = 4096;

/// Default maximum retries for ENQCMD.
const DEFAULT_MAX_RETRIES: u32 = 1000;

/// Default spin iterations while waiting for completion.
const DEFAULT_SPIN_ITERATIONS: u32 = 1_000_000;

/// Work queue type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkQueueType {
    /// Dedicated Work Queue - single user, uses MOVDIR64B.
    Dedicated,
    /// Shared Work Queue - multiple users, uses ENQCMD with PASID.
    Shared,
}

/// Information about a work queue (from sysfs).
#[derive(Debug, Clone)]
pub struct WorkQueueInfo {
    /// Work queue name (e.g., "wq0.0").
    pub name: String,
    /// State ("enabled", "disabled", etc.).
    pub state: String,
    /// Work queue type.
    pub wq_type: WorkQueueType,
    /// Queue size (number of entries).
    pub size: u32,
    /// Threshold for shared WQ.
    pub threshold: u32,
}

// ============================================================================
// Linux Implementation
// ============================================================================

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;

    /// Handle to an open work queue.
    ///
    /// This struct manages the lifecycle of a work queue, including:
    /// - The file descriptor to the character device
    /// - The memory-mapped portal for descriptor submission
    pub struct WorkQueue {
        /// File handle to the work queue device.
        #[allow(dead_code)]
        file: File,
        /// Memory-mapped portal address.
        portal: *mut u8,
        /// Portal mapping size.
        portal_size: usize,
        /// Work queue type (determines submission method).
        wq_type: WorkQueueType,
        /// Maximum retries for ENQCMD.
        max_retries: u32,
        /// Spin iterations for completion polling.
        spin_iterations: u32,
    }

    // SAFETY: WorkQueue can be sent between threads because:
    // - The file descriptor is owned and valid
    // - The portal pointer is valid for the lifetime of the mapping
    // - DSA operations are thread-safe at the hardware level
    unsafe impl Send for WorkQueue {}

    // SAFETY: WorkQueue can be shared between threads with proper synchronization
    // The caller must ensure only one descriptor is in flight per completion record
    unsafe impl Sync for WorkQueue {}

    impl WorkQueue {
        /// Open a work queue device.
        ///
        /// # Arguments
        ///
        /// * `path` - Path to the work queue device (e.g., `/dev/dsa/wq0.0`)
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The device cannot be opened (permissions, not found)
        /// - Memory mapping fails
        pub fn open(path: &Path) -> Result<Self, DsaError> {
            // Open the work queue character device
            let file = File::options()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        DsaError::PermissionDenied(path.display().to_string())
                    } else {
                        DsaError::Io(e)
                    }
                })?;

            // Memory-map the portal
            let portal = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    PORTAL_SIZE,
                    libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    file.as_raw_fd(),
                    0,
                )
            };

            if portal == libc::MAP_FAILED {
                return Err(DsaError::MmapFailed(format!(
                    "mmap failed for {}",
                    path.display()
                )));
            }

            // TODO: Detect WQ type from sysfs or device properties
            // For now, default to Shared (more common for user-space)
            let wq_type = WorkQueueType::Shared;

            Ok(Self {
                file,
                portal: portal as *mut u8,
                portal_size: PORTAL_SIZE,
                wq_type,
                max_retries: DEFAULT_MAX_RETRIES,
                spin_iterations: DEFAULT_SPIN_ITERATIONS,
            })
        }

        /// Set the work queue type.
        pub fn set_wq_type(&mut self, wq_type: WorkQueueType) {
            self.wq_type = wq_type;
        }

        /// Set the maximum retries for ENQCMD submissions.
        pub fn set_max_retries(&mut self, retries: u32) {
            self.max_retries = retries;
        }

        /// Set the spin iterations for completion polling.
        pub fn set_spin_iterations(&mut self, iterations: u32) {
            self.spin_iterations = iterations;
        }

        /// Get the work queue type.
        pub fn wq_type(&self) -> WorkQueueType {
            self.wq_type
        }

        /// Submit a descriptor to the work queue.
        ///
        /// # Safety
        ///
        /// The completion record in the descriptor must remain valid until
        /// the operation completes.
        unsafe fn submit(&self, desc: &DsaHwDesc) -> Result<(), DsaError> {
            match self.wq_type {
                WorkQueueType::Dedicated => {
                    movdir64b(self.portal, desc);
                    Ok(())
                }
                WorkQueueType::Shared => {
                    if enqcmd_retry(self.portal, desc, self.max_retries) {
                        Ok(())
                    } else {
                        Err(DsaError::QueueFull)
                    }
                }
            }
        }

        /// Wait for a completion record to be filled.
        fn wait_for_completion(&self, record: &DsaCompletionRecord) -> Result<(), DsaError> {
            for _ in 0..self.spin_iterations {
                if record.is_complete() {
                    let status = record.get_status();
                    return match status {
                        CompletionStatus::Success => Ok(()),
                        CompletionStatus::PageFault => Err(DsaError::PageFault {
                            fault_addr: record.fault_addr,
                            bytes_completed: record.bytes_completed,
                        }),
                        _ => Err(DsaError::OperationFailed {
                            status: record.status,
                            result: record.result,
                        }),
                    };
                }
                core::hint::spin_loop();
            }

            // Timeout - operation didn't complete in time
            Err(DsaError::OperationFailed {
                status: 0,
                result: 0,
            })
        }

        /// Compute CRC32 checksum of data.
        pub fn crc32(&self, data: &[u8], seed: u32) -> Result<u32, DsaError> {
            if data.is_empty() {
                return Ok(seed);
            }

            let mut completion = DsaCompletionRecord::new();
            let desc = DsaHwDesc::crc_gen(data.as_ptr(), data.len(), seed, &mut completion);

            unsafe { self.submit(&desc)? };
            self.wait_for_completion(&completion)?;

            Ok(completion.crc32_result())
        }

        /// Copy memory from source to destination.
        pub fn memcpy(&self, dst: &mut [u8], src: &[u8]) -> Result<(), DsaError> {
            if dst.len() < src.len() {
                return Err(DsaError::BufferSizeMismatch {
                    expected: src.len(),
                    actual: dst.len(),
                });
            }

            if src.is_empty() {
                return Ok(());
            }

            let mut completion = DsaCompletionRecord::new();
            let desc =
                DsaHwDesc::mem_move(dst.as_mut_ptr(), src.as_ptr(), src.len(), &mut completion);

            unsafe { self.submit(&desc)? };
            self.wait_for_completion(&completion)
        }

        /// Fill memory with a 64-bit pattern.
        pub fn memset(&self, dst: &mut [u8], pattern: u64) -> Result<(), DsaError> {
            if dst.is_empty() {
                return Ok(());
            }

            let mut completion = DsaCompletionRecord::new();
            let desc = DsaHwDesc::mem_fill(dst.as_mut_ptr(), dst.len(), pattern, &mut completion);

            unsafe { self.submit(&desc)? };
            self.wait_for_completion(&completion)
        }

        /// Compare two memory regions.
        pub fn memcmp(&self, a: &[u8], b: &[u8]) -> Result<bool, DsaError> {
            if a.len() != b.len() {
                return Err(DsaError::BufferSizeMismatch {
                    expected: a.len(),
                    actual: b.len(),
                });
            }

            if a.is_empty() {
                return Ok(true);
            }

            let mut completion = DsaCompletionRecord::new();
            let desc = DsaHwDesc::compare(a.as_ptr(), b.as_ptr(), a.len(), &mut completion);

            unsafe { self.submit(&desc)? };
            self.wait_for_completion(&completion)?;

            Ok(completion.compare_result())
        }

        /// Execute a no-op operation (for testing/benchmarking).
        pub fn noop(&self) -> Result<(), DsaError> {
            let mut completion = DsaCompletionRecord::new();
            let desc = DsaHwDesc::noop(&mut completion);

            unsafe { self.submit(&desc)? };
            self.wait_for_completion(&completion)
        }
    }

    impl Drop for WorkQueue {
        fn drop(&mut self) {
            unsafe {
                libc::munmap(self.portal as *mut libc::c_void, self.portal_size);
            }
        }
    }
}

// ============================================================================
// Non-Linux Stub Implementation
// ============================================================================

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;

    /// Software-based work queue for Windows.
    ///
    /// On Windows, hardware DSA access is not available through userspace APIs.
    /// Intel's own DML library also uses software fallback on Windows.
    /// This implementation provides optimized software implementations for:
    /// - CRC32 (using crc32fast which uses SIMD when available)
    /// - Memory operations (using optimized std library functions)
    ///
    /// While not as fast as hardware DSA, these implementations are still
    /// highly optimized and significantly faster than naive implementations.
    pub struct WorkQueue {
        /// Indicates this is a software-only work queue
        is_software: bool,
        /// CRC32 hasher for software fallback
        crc_hasher: crc32fast::Hasher,
    }

    impl WorkQueue {
        /// Open a software-emulated work queue.
        ///
        /// On Windows, this always creates a software fallback work queue
        /// since hardware DSA access is not available.
        pub fn open(_path: &Path) -> Result<Self, DsaError> {
            log::info!("Opening software-emulated DSA work queue (Windows)");
            Ok(Self {
                is_software: true,
                crc_hasher: crc32fast::Hasher::new(),
            })
        }

        pub fn set_wq_type(&mut self, _wq_type: WorkQueueType) {}
        pub fn set_max_retries(&mut self, _retries: u32) {}
        pub fn set_spin_iterations(&mut self, _iterations: u32) {}

        pub fn wq_type(&self) -> WorkQueueType {
            WorkQueueType::Shared
        }

        /// Returns true if this is a software-emulated work queue.
        pub fn is_software_fallback(&self) -> bool {
            self.is_software
        }

        /// Compute CRC32 checksum using crc32fast (SIMD-accelerated).
        ///
        /// Uses the IEEE polynomial (same as DSA hardware).
        pub fn crc32(&self, data: &[u8], seed: u32) -> Result<u32, DsaError> {
            if data.is_empty() {
                return Ok(seed);
            }

            let mut hasher = crc32fast::Hasher::new_with_initial(seed);
            hasher.update(data);
            Ok(hasher.finalize())
        }

        /// Copy memory using optimized standard library copy.
        pub fn memcpy(&self, dst: &mut [u8], src: &[u8]) -> Result<(), DsaError> {
            if dst.len() < src.len() {
                return Err(DsaError::BufferSizeMismatch {
                    expected: src.len(),
                    actual: dst.len(),
                });
            }

            if src.is_empty() {
                return Ok(());
            }

            dst[..src.len()].copy_from_slice(src);
            Ok(())
        }

        /// Fill memory with a 64-bit pattern.
        pub fn memset(&self, dst: &mut [u8], pattern: u64) -> Result<(), DsaError> {
            if dst.is_empty() {
                return Ok(());
            }

            let pattern_bytes = pattern.to_le_bytes();

            // Fill using the 8-byte pattern
            for chunk in dst.chunks_exact_mut(8) {
                chunk.copy_from_slice(&pattern_bytes);
            }

            // Handle remaining bytes
            let remainder = dst.len() % 8;
            if remainder > 0 {
                let start = dst.len() - remainder;
                dst[start..].copy_from_slice(&pattern_bytes[..remainder]);
            }

            Ok(())
        }

        /// Compare two memory regions.
        pub fn memcmp(&self, a: &[u8], b: &[u8]) -> Result<bool, DsaError> {
            if a.len() != b.len() {
                return Err(DsaError::BufferSizeMismatch {
                    expected: a.len(),
                    actual: b.len(),
                });
            }

            Ok(a == b)
        }

        /// No-op operation (completes immediately for software fallback).
        pub fn noop(&self) -> Result<(), DsaError> {
            Ok(())
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod stub_impl {
    use super::*;

    /// Stub work queue for non-Linux platforms.
    ///
    /// All operations return `DsaError::PlatformNotSupported`.
    pub struct WorkQueue {
        _private: (),
    }

    impl WorkQueue {
        /// Attempting to open a work queue on non-Linux returns an error.
        pub fn open(_path: &Path) -> Result<Self, DsaError> {
            Err(DsaError::PlatformNotSupported)
        }

        pub fn set_wq_type(&mut self, _wq_type: WorkQueueType) {}
        pub fn set_max_retries(&mut self, _retries: u32) {}
        pub fn set_spin_iterations(&mut self, _iterations: u32) {}
        pub fn wq_type(&self) -> WorkQueueType {
            WorkQueueType::Shared
        }

        pub fn crc32(&self, _data: &[u8], _seed: u32) -> Result<u32, DsaError> {
            Err(DsaError::PlatformNotSupported)
        }

        pub fn memcpy(&self, _dst: &mut [u8], _src: &[u8]) -> Result<(), DsaError> {
            Err(DsaError::PlatformNotSupported)
        }

        pub fn memset(&self, _dst: &mut [u8], _pattern: u64) -> Result<(), DsaError> {
            Err(DsaError::PlatformNotSupported)
        }

        pub fn memcmp(&self, _a: &[u8], _b: &[u8]) -> Result<bool, DsaError> {
            Err(DsaError::PlatformNotSupported)
        }

        pub fn noop(&self) -> Result<(), DsaError> {
            Err(DsaError::PlatformNotSupported)
        }
    }
}

// Re-export the appropriate implementation
#[cfg(target_os = "linux")]
pub use linux_impl::WorkQueue;

#[cfg(target_os = "windows")]
pub use windows_impl::WorkQueue;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use stub_impl::WorkQueue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_queue_type() {
        assert_eq!(WorkQueueType::Dedicated, WorkQueueType::Dedicated);
        assert_ne!(WorkQueueType::Dedicated, WorkQueueType::Shared);
    }

    #[test]
    fn test_work_queue_info() {
        let info = WorkQueueInfo {
            name: "wq0.0".to_string(),
            state: "enabled".to_string(),
            wq_type: WorkQueueType::Shared,
            size: 128,
            threshold: 64,
        };

        assert_eq!(info.name, "wq0.0");
        assert_eq!(info.state, "enabled");
        assert_eq!(info.wq_type, WorkQueueType::Shared);
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    #[test]
    fn test_stub_returns_platform_not_supported() {
        use std::path::PathBuf;
        let result = WorkQueue::open(&PathBuf::from("/dev/dsa/wq0.0"));
        assert!(matches!(result, Err(DsaError::PlatformNotSupported)));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_software_fallback() {
        use std::path::PathBuf;
        let result = WorkQueue::open(&PathBuf::from(""));
        assert!(
            result.is_ok(),
            "Windows should return software fallback WorkQueue"
        );

        let wq = result.unwrap();
        assert!(wq.is_software_fallback());

        // Test CRC32 - just verify it produces a value (exact value depends on implementation)
        let crc = wq.crc32(b"Hello, DSA!", 0).unwrap();
        assert_ne!(crc, 0); // Should produce a non-zero CRC

        // Verify CRC consistency - same input should produce same output
        let crc2 = wq.crc32(b"Hello, DSA!", 0).unwrap();
        assert_eq!(crc, crc2);

        // Test memcpy
        let src = vec![0xABu8; 64];
        let mut dst = vec![0u8; 64];
        wq.memcpy(&mut dst, &src).unwrap();
        assert_eq!(src, dst);

        // Test memset
        let mut buf = vec![0u8; 16];
        wq.memset(&mut buf, 0xDEADBEEF_CAFEBABE).unwrap();
        assert_eq!(&buf[..8], &[0xBE, 0xBA, 0xFE, 0xCA, 0xEF, 0xBE, 0xAD, 0xDE]);

        // Test memcmp
        let a = vec![1u8; 64];
        let b = vec![1u8; 64];
        let c = vec![2u8; 64];
        assert!(wq.memcmp(&a, &b).unwrap());
        assert!(!wq.memcmp(&a, &c).unwrap());
    }
}
