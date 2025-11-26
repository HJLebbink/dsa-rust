// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! Basic example demonstrating Intel DSA usage.
//!
//! Run with: `cargo run --example basic`

use dsa_rust::{discover_devices, is_dsa_available, is_dsa_configured, DsaEngine, DsaError};

fn main() {
    println!("Intel DSA Basic Example");
    println!("=======================\n");

    // Check platform support
    println!("Checking DSA availability...");
    println!("  DSA hardware detected: {}", is_dsa_available());
    println!("  DSA configured: {}", is_dsa_configured());

    #[cfg(target_os = "windows")]
    println!("  Note: On Windows, software fallback is used (hardware access unavailable)");
    println!();

    // Try to discover devices
    println!("Discovering DSA devices...");
    match discover_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  No DSA hardware found.");
                #[cfg(target_os = "windows")]
                println!("  Software fallback will be used.");
            } else {
                for device in &devices {
                    println!("  Device: {}", device.name);
                    println!("    Path: {}", device.sysfs_path.display());
                    println!("    Work queues: {}", device.wq_count());
                    println!("    Enabled WQs: {}", device.enabled_wq_count());
                    for wq in &device.work_queues {
                        println!(
                            "      - {} (state: {}, type: {:?})",
                            wq.name, wq.state, wq.wq_type
                        );
                    }
                }
            }
        }
        Err(DsaError::PlatformNotSupported) => {
            println!("  Platform not supported.");
            return;
        }
        Err(e) => {
            println!("  Error discovering devices: {}", e);
            // On Windows, continue even if discovery fails - we can use software fallback
            #[cfg(not(target_os = "windows"))]
            return;
        }
    }
    println!();

    // Try to open DSA engine and perform operations
    println!("Opening DSA engine...");
    let engine = match DsaEngine::open_first() {
        Ok(engine) => {
            #[cfg(target_os = "linux")]
            println!("  Successfully opened hardware DSA engine!");
            #[cfg(target_os = "windows")]
            println!("  Successfully opened software-emulated DSA engine!");
            engine
        }
        Err(e) => {
            println!("  Failed to open DSA engine: {}", e);
            return;
        }
    };
    println!();

    // CRC32 example
    println!("Computing CRC32...");
    let test_data = b"Hello, Intel DSA!";
    match engine.crc32(test_data) {
        Ok(crc) => println!(
            "  CRC32 of {:?}: {:#010x}",
            String::from_utf8_lossy(test_data),
            crc
        ),
        Err(e) => println!("  CRC32 failed: {}", e),
    }
    println!();

    // Memory copy example
    println!("Testing memory copy...");
    let src = vec![0xABu8; 4096];
    let mut dst = vec![0u8; 4096];
    match engine.memcpy(&mut dst, &src) {
        Ok(()) => {
            let matches = src == dst;
            println!("  Copied 4KB, data matches: {}", matches);
        }
        Err(e) => println!("  Memory copy failed: {}", e),
    }
    println!();

    // Memory fill example
    println!("Testing memory fill...");
    let mut buffer = vec![0u8; 4096];
    let pattern = 0xDEADBEEFCAFEBABEu64;
    match engine.memset(&mut buffer, pattern) {
        Ok(()) => {
            println!("  Filled 4KB with pattern {:#018x}", pattern);
            println!("  First 8 bytes: {:02x?}", &buffer[0..8]);
        }
        Err(e) => println!("  Memory fill failed: {}", e),
    }
    println!();

    // Memory compare example
    println!("Testing memory compare...");
    let a = vec![1u8; 4096];
    let b = vec![1u8; 4096];
    let c = vec![2u8; 4096];
    match engine.memcmp(&a, &b) {
        Ok(equal) => println!("  a == b: {}", equal),
        Err(e) => println!("  Compare failed: {}", e),
    }
    match engine.memcmp(&a, &c) {
        Ok(equal) => println!("  a == c: {}", equal),
        Err(e) => println!("  Compare failed: {}", e),
    }
    println!();

    println!("Done!");
}
