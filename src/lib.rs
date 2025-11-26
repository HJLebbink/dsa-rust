// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! # Intel DSA (Data Streaming Accelerator) Rust Bindings
//!
//! This crate provides safe Rust bindings for Intel's Data Streaming Accelerator,
//! available on Intel Xeon Scalable processors (4th Gen "Sapphire Rapids" and later).
//!
//! ## Supported Operations
//!
//! - CRC32 generation
//! - Memory copy (memcpy)
//! - Memory fill (memset)
//! - Memory compare (memcmp)
//! - Batch operations
//!
//! ## Platform Support
//!
//! | Platform | Hardware DSA | Software Fallback |
//! |----------|--------------|-------------------|
//! | Linux    | Supported    | Not needed        |
//! | Windows  | Not available| Supported         |
//! | WSL2     | Not available| N/A               |
//!
//! ### Windows
//!
//! On Windows, hardware DSA access is not available through userspace APIs.
//! This crate provides optimized software fallback using `crc32fast` (SIMD-accelerated)
//! and standard library memory operations.
//!
//! ### WSL2 Limitations
//!
//! DSA does **not** work on WSL2 because:
//! - WSL2 uses a virtualized kernel without direct hardware access
//! - The IDXD driver and `/dev/dsa` devices are not available
//! - The Hyper-V hypervisor blocks direct DSA access
//!
//! For hardware DSA acceleration, use native Linux (bare metal or dual boot).
//!
//! ## Example
//!
//! ```rust,no_run
//! use dsa_rust::{DsaEngine, DsaError};
//!
//! fn main() -> Result<(), DsaError> {
//!     // Discover and open first available DSA device
//!     let engine = DsaEngine::open_first()?;
//!
//!     // Compute CRC32 using hardware acceleration
//!     let data = b"Hello, DSA!";
//!     let crc = engine.crc32(data)?;
//!
//!     println!("CRC32: {:#010x}", crc);
//!     Ok(())
//! }
//! ```
//!
//! ## Requirements
//!
//! ### Hardware
//! - Intel Xeon Scalable 4th Gen (Sapphire Rapids) or later
//! - Intel Xeon 6 (Granite Rapids)
//! - Intel Xeon w5-2400 series (Sapphire Rapids workstation)
//!
//! ### Software (Linux - hardware acceleration)
//! - Linux kernel 5.11+ with IDXD driver enabled
//! - DSA device configured via `accel-config`
//!
//! ### Software (Windows - software fallback)
//! - Windows 10/11 or Windows Server 2019+
//! - No additional configuration required

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(target_arch = "x86_64")]
#![allow(dead_code)] // During development

#[cfg(feature = "std")]
extern crate std;

// Module declarations
pub mod descriptor;
pub mod device;
pub mod engine;
pub mod error;
pub mod opcode;
pub mod submit;
pub mod wq;

// Re-exports for convenient access
pub use descriptor::{CompletionStatus, DsaCompletionRecord, DsaHwDesc};
pub use device::{discover_devices, is_dsa_available, is_dsa_configured, DsaDevice};
pub use engine::DsaEngine;
pub use error::DsaError;
pub use opcode::DsaOpcode;
pub use wq::{WorkQueue, WorkQueueType};
