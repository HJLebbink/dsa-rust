# Claude Code Style Guide for dsa-rust

## Project Overview

Rust bindings for Intel Data Streaming Accelerator (DSA), a hardware accelerator
available on Intel Xeon Scalable processors (4th Gen Sapphire Rapids and later).

## Copyright Header

All source files MUST include the following copyright header:

```rust
// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT
```

## Code Style

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Prefer safe Rust; document all `unsafe` blocks with safety comments
- Use `#[repr(C)]` for structures that map to hardware layouts
- Include compile-time size assertions for hardware structures

## Platform Support

- **Linux**: Primary platform, uses IDXD driver via `/dev/dsa/`
- **Windows**: Secondary platform, uses Windows DSA APIs (work in progress)

Use `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "windows")]` for
platform-specific code. Provide stub implementations for unsupported platforms.

## Testing

- Unit tests should not require hardware
- Integration tests requiring DSA should be skipped if hardware unavailable
- Use `#[cfg(test)]` for test modules

## Build Commands

```bash
cargo build              # Build library
cargo test               # Run tests
cargo bench              # Run benchmarks (requires DSA hardware)
cargo clippy             # Lint check
cargo fmt                # Format code
```

## Architecture Notes

### Key Structures

- `DsaHwDesc` - 64-byte hardware descriptor (must be 64-byte aligned)
- `DsaCompletionRecord` - 64-byte completion record (must be 32-byte aligned)
- `DsaOpcode` - Enum of DSA operation codes

### Submission Instructions

- **MOVDIR64B** - For Dedicated Work Queues (posted write)
- **ENQCMD** - For Shared Work Queues (non-posted, returns success/busy)

### Work Queue Types

- **Dedicated (DWQ)** - Single application, lower overhead
- **Shared (SWQ)** - Multiple applications, requires PASID
