# dsa-rust

Rust bindings for Intel Data Streaming Accelerator (DSA).

## Overview

Intel DSA is a hardware accelerator available on Intel Xeon Scalable processors
(4th Gen "Sapphire Rapids" and later). It offloads data movement and transformation
operations from CPU cores to dedicated silicon.

This crate provides safe Rust bindings to DSA, enabling:

- **CRC32 generation** - Hardware-accelerated checksums
- **Memory copy** - DMA-like memory transfers
- **Memory fill** - Pattern fills
- **Memory compare** - Buffer comparison

## Requirements

### Hardware
- Intel Xeon Scalable 4th Gen (Sapphire Rapids) or later
- Intel Xeon 6 (Granite Rapids)
- Intel Xeon w5-2400 series (Sapphire Rapids workstation)

### Software

**Linux (hardware acceleration):**
- Linux kernel 5.11+ with IDXD driver enabled
- DSA device configured via `accel-config`

**Windows (software fallback):**
- Windows 10/11 or Windows Server 2019+
- No additional configuration required

## Quick Start

```rust
use dsa_rust::{DsaEngine, DsaError};

fn main() -> Result<(), DsaError> {
    // Open the first available DSA device
    let engine = DsaEngine::open_first()?;

    // Compute CRC32 using hardware acceleration
    let data = b"Hello, DSA!";
    let crc = engine.crc32(data)?;
    println!("CRC32: {:#010x}", crc);

    // Memory copy
    let src = vec![1u8; 4096];
    let mut dst = vec![0u8; 4096];
    engine.memcpy(&mut dst, &src)?;

    Ok(())
}
```

## Device Configuration

Before using DSA, the device must be configured using `accel-config`:

```bash
# Install accel-config
sudo apt install accel-config  # Debian/Ubuntu
# or build from source: https://github.com/intel/idxd-config

# List available devices
accel-config list

# Configure a work queue (example)
sudo accel-config config-wq dsa0/wq0.0 \
    --mode=shared \
    --type=user \
    --size=16 \
    --priority=10 \
    --name=app_wq

# Enable the device and work queue
sudo accel-config enable-device dsa0
sudo accel-config enable-wq dsa0/wq0.0

# Set permissions (or add user to appropriate group)
sudo chmod 666 /dev/dsa/wq0.0
```

## Work Queue Types

DSA supports two types of work queues:

### Dedicated Work Queue (DWQ)
- Single user/application
- Uses MOVDIR64B instruction
- Lower submission overhead
- No retry needed

### Shared Work Queue (SWQ)
- Multiple users/applications
- Uses ENQCMD instruction
- Requires PASID (Process Address Space ID)
- May need retry if queue is full

## Performance Considerations

DSA provides the best speedup for:

- Large buffers (>= 4KB)
- Operations where CPU can do other work during DSA processing
- Batch operations (multiple descriptors submitted at once)

For small buffers (< 4KB), software implementations may be faster due to
DSA submission overhead.

## Features

- `std` (default) - Standard library support
- `async` - Async/await support with Tokio

## Platform Support

| Platform | Hardware DSA | Software Fallback |
|----------|--------------|-------------------|
| Linux    | Supported    | Not needed        |
| Windows  | Not available| Supported         |
| WSL2     | Not available| N/A               |

### Windows Implementation Details

On Windows, hardware DSA access is not available through userspace APIs. Intel's own
Data Mover Library (DML) also uses software fallback on Windows for this reason.

This crate provides:
- **Device detection** via SetupAPI (detects DSA hardware presence)
- **Software fallback** using optimized implementations:
  - CRC32: Uses `crc32fast` crate (SIMD-accelerated)
  - Memory operations: Uses optimized standard library functions

While not as fast as hardware DSA, the software implementations are highly optimized
and provide a consistent API across platforms.

### WSL2 Limitations

DSA does not work on WSL2 because:
- WSL2 uses a virtualized kernel without direct hardware access
- The IDXD driver and `/dev/dsa` devices are not available
- The Hyper-V hypervisor blocks direct DSA access

For hardware DSA acceleration, use native Linux (bare metal or dual boot).

## License

Licensed under the MIT license. See [LICENSE](LICENSE) for details.

## References

- [Intel DSA Architecture Specification](https://www.intel.com/content/www/us/en/content-details/857060/intel-data-streaming-accelerator-architecture-specification.html)
- [Linux IDXD Driver](https://github.com/torvalds/linux/tree/master/drivers/dma/idxd)
- [accel-config (IDXD Configuration Tool)](https://github.com/intel/idxd-config)
- [A Quantitative Analysis of DSA](https://arxiv.org/abs/2305.02480)
