// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! DSA device discovery.
//!
//! # Platform Support
//!
//! ## Linux
//! On Linux, DSA devices appear in `/sys/bus/dsa/devices/` with entries like:
//! - `dsa0`, `dsa1`, ... - DSA device instances
//! - `wq0.0`, `wq0.1`, ... - Work queues on device 0
//!
//! Work queue character devices appear at `/dev/dsa/wq0.0`, etc.
//!
//! ## Windows
//! Windows support is planned but not yet implemented.

use crate::error::DsaError;
use crate::wq::{WorkQueue, WorkQueueInfo};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use std::fs;

/// Sysfs base path for DSA devices (Linux only).
#[cfg(target_os = "linux")]
const SYSFS_DSA_PATH: &str = "/sys/bus/dsa/devices";

/// Device node base path for DSA work queues (Linux only).
#[cfg(target_os = "linux")]
const DEV_DSA_PATH: &str = "/dev/dsa";

/// Information about a DSA device.
#[derive(Debug, Clone)]
pub struct DsaDevice {
    /// Device name (e.g., "dsa0").
    pub name: String,
    /// Sysfs path for this device (Linux) or device path (Windows).
    pub sysfs_path: PathBuf,
    /// Available work queues on this device.
    pub work_queues: Vec<WorkQueueInfo>,
}

impl DsaDevice {
    /// Open the first available enabled work queue on this device.
    #[cfg(target_os = "linux")]
    pub fn open_first_wq(&self) -> Result<WorkQueue, DsaError> {
        for wq_info in &self.work_queues {
            if wq_info.state == "enabled" {
                let dev_path = Path::new(DEV_DSA_PATH).join(&wq_info.name);
                if dev_path.exists() {
                    return WorkQueue::open(&dev_path);
                }
            }
        }
        Err(DsaError::NoWorkQueue)
    }

    /// Open the first available enabled work queue on this device.
    #[cfg(not(target_os = "linux"))]
    pub fn open_first_wq(&self) -> Result<WorkQueue, DsaError> {
        Err(DsaError::PlatformNotSupported)
    }

    /// Open a specific work queue by name (e.g., "wq0.0").
    #[cfg(target_os = "linux")]
    pub fn open_wq(&self, name: &str) -> Result<WorkQueue, DsaError> {
        let dev_path = Path::new(DEV_DSA_PATH).join(name);
        WorkQueue::open(&dev_path)
    }

    /// Open a specific work queue by name.
    #[cfg(not(target_os = "linux"))]
    pub fn open_wq(&self, _name: &str) -> Result<WorkQueue, DsaError> {
        Err(DsaError::PlatformNotSupported)
    }

    /// Get the number of available work queues.
    pub fn wq_count(&self) -> usize {
        self.work_queues.len()
    }

    /// Get the number of enabled work queues.
    pub fn enabled_wq_count(&self) -> usize {
        self.work_queues
            .iter()
            .filter(|wq| wq.state == "enabled")
            .count()
    }
}

// ============================================================================
// Linux Implementation
// ============================================================================

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;

    pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
        let sysfs_path = Path::new(SYSFS_DSA_PATH);

        if !sysfs_path.exists() {
            return Err(DsaError::PlatformNotSupported);
        }

        let mut devices = Vec::new();
        let entries = fs::read_dir(sysfs_path)?;

        let mut device_names: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("dsa") && !name.contains('.') {
                device_names.push(name);
            }
        }

        for device_name in device_names {
            let device_sysfs = sysfs_path.join(&device_name);
            let work_queues = discover_work_queues(&device_name)?;

            devices.push(DsaDevice {
                name: device_name,
                sysfs_path: device_sysfs,
                work_queues,
            });
        }

        Ok(devices)
    }

    fn discover_work_queues(device_name: &str) -> Result<Vec<WorkQueueInfo>, DsaError> {
        let sysfs_path = Path::new(SYSFS_DSA_PATH);
        let mut work_queues = Vec::new();

        let device_num = device_name
            .strip_prefix("dsa")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let entries = fs::read_dir(sysfs_path)?;

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with(&format!("wq{}.", device_num)) {
                let wq_path = sysfs_path.join(&name);
                let wq_info = read_wq_info(&name, &wq_path)?;
                work_queues.push(wq_info);
            }
        }

        work_queues.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(work_queues)
    }

    fn read_wq_info(name: &str, path: &Path) -> Result<WorkQueueInfo, DsaError> {
        let state =
            read_sysfs_string(&path.join("state")).unwrap_or_else(|_| "unknown".to_string());
        let mode = read_sysfs_string(&path.join("mode")).unwrap_or_else(|_| "unknown".to_string());
        let size = read_sysfs_u32(&path.join("size")).unwrap_or(0);
        let threshold = read_sysfs_u32(&path.join("threshold")).unwrap_or(0);

        let wq_type = match mode.as_str() {
            "dedicated" => WorkQueueType::Dedicated,
            "shared" => WorkQueueType::Shared,
            _ => WorkQueueType::Shared,
        };

        Ok(WorkQueueInfo {
            name: name.to_string(),
            state,
            wq_type,
            size,
            threshold,
        })
    }

    fn read_sysfs_string(path: &Path) -> Result<String, DsaError> {
        Ok(fs::read_to_string(path)?.trim().to_string())
    }

    fn read_sysfs_u32(path: &Path) -> Result<u32, DsaError> {
        let s = read_sysfs_string(path)?;
        s.parse()
            .map_err(|_| DsaError::InvalidArgument(format!("invalid u32 in sysfs: {}", s)))
    }

    pub fn is_dsa_available() -> bool {
        Path::new(SYSFS_DSA_PATH).exists()
    }

    pub fn is_dsa_configured() -> bool {
        Path::new(DEV_DSA_PATH).exists()
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use crate::wq::WorkQueueType;
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
        SetupDiGetDeviceRegistryPropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, SPDRP_DEVICEDESC,
        SPDRP_HARDWAREID, SP_DEVINFO_DATA,
    };
    use windows::Win32::Foundation::ERROR_NO_MORE_ITEMS;

    /// Intel DSA PCI Vendor and Device IDs
    /// VID: 8086 (Intel), DID: 0B25 (DSA)
    const INTEL_DSA_HARDWARE_ID_PREFIX: &str = "PCI\\VEN_8086&DEV_0B25";

    pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
        let mut devices = Vec::new();

        // Get handle to device information set for all present devices
        let dev_info = unsafe {
            SetupDiGetClassDevsW(
                None,           // All device classes
                PCWSTR::null(), // No enumerator filter
                None,           // No parent window
                DIGCF_ALLCLASSES | DIGCF_PRESENT,
            )
        }
        .map_err(|e| DsaError::Io(std::io::Error::from_raw_os_error(e.code().0)))?;

        // Use a simple guard struct instead of scopeguard for cleanup
        struct DevInfoCleanup(windows::Win32::Devices::DeviceAndDriverInstallation::HDEVINFO);
        impl Drop for DevInfoCleanup {
            fn drop(&mut self) {
                unsafe {
                    let _ = SetupDiDestroyDeviceInfoList(self.0);
                }
            }
        }
        let _cleanup = DevInfoCleanup(dev_info);

        let mut dev_info_data = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };

        let mut device_index = 0u32;
        let mut dsa_count = 0u32;

        loop {
            // Enumerate each device
            let result =
                unsafe { SetupDiEnumDeviceInfo(dev_info, device_index, &mut dev_info_data) };

            if result.is_err() {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(ERROR_NO_MORE_ITEMS.0 as i32) {
                    break;
                }
                device_index += 1;
                continue;
            }

            // Get hardware ID
            let mut hardware_id_buffer = vec![0u8; 512];
            let mut required_size = 0u32;

            let hw_result = unsafe {
                SetupDiGetDeviceRegistryPropertyW(
                    dev_info,
                    &dev_info_data,
                    SPDRP_HARDWAREID,
                    None,
                    Some(&mut hardware_id_buffer),
                    Some(&mut required_size),
                )
            };

            if hw_result.is_ok() {
                // Convert to string and check for DSA
                let hardware_id = String::from_utf16_lossy(
                    &hardware_id_buffer
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .take_while(|&c| c != 0)
                        .collect::<Vec<u16>>(),
                );

                if hardware_id
                    .to_uppercase()
                    .contains(&INTEL_DSA_HARDWARE_ID_PREFIX.to_uppercase())
                {
                    // Found a DSA device - get its description
                    let mut desc_buffer = vec![0u8; 256];
                    let mut desc_required = 0u32;

                    let description = if unsafe {
                        SetupDiGetDeviceRegistryPropertyW(
                            dev_info,
                            &dev_info_data,
                            SPDRP_DEVICEDESC,
                            None,
                            Some(&mut desc_buffer),
                            Some(&mut desc_required),
                        )
                    }
                    .is_ok()
                    {
                        String::from_utf16_lossy(
                            &desc_buffer
                                .chunks_exact(2)
                                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                .take_while(|&c| c != 0)
                                .collect::<Vec<u16>>(),
                        )
                    } else {
                        format!("Intel DSA Device {}", dsa_count)
                    };

                    devices.push(DsaDevice {
                        name: format!("dsa{}", dsa_count),
                        sysfs_path: PathBuf::from(format!("\\\\.\\DSA{}", dsa_count)),
                        work_queues: vec![WorkQueueInfo {
                            name: format!("wq{}.0", dsa_count),
                            state: "software".to_string(), // Hardware access not available
                            wq_type: WorkQueueType::Shared,
                            size: 128,
                            threshold: 64,
                        }],
                    });

                    log::info!("Found Intel DSA device: {} ({})", description, hardware_id);
                    dsa_count += 1;
                }
            }

            device_index += 1;
        }

        if devices.is_empty() {
            // No hardware found, but we can still provide software fallback
            // Return empty list - callers should use software fallback
            Ok(devices)
        } else {
            Ok(devices)
        }
    }

    pub fn is_dsa_available() -> bool {
        // Check if DSA hardware is present via device enumeration
        discover_devices()
            .map(|devices| !devices.is_empty())
            .unwrap_or(false)
    }

    pub fn is_dsa_configured() -> bool {
        // On Windows, "configured" means hardware is present
        // (no equivalent to Linux IDXD driver configuration)
        is_dsa_available()
    }
}

// ============================================================================
// Unsupported Platform Stub
// ============================================================================

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod stub_impl {
    use super::*;

    pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
        Err(DsaError::PlatformNotSupported)
    }

    pub fn is_dsa_available() -> bool {
        false
    }

    pub fn is_dsa_configured() -> bool {
        false
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Discover all DSA devices on the system.
///
/// # Platform Support
///
/// - **Linux**: Scans `/sys/bus/dsa/devices/` for DSA devices
/// - **Windows**: Not yet implemented (returns `PlatformNotSupported`)
///
/// # Errors
///
/// Returns an error if:
/// - The platform is not supported
/// - The sysfs DSA path doesn't exist (Linux: no IDXD driver)
/// - Failed to read device information
///
/// # Example
///
/// ```rust,no_run
/// use intel_dsa::discover_devices;
///
/// let devices = discover_devices()?;
/// for device in &devices {
///     println!("Found DSA device: {}", device.name);
///     for wq in &device.work_queues {
///         println!("  Work queue: {} ({})", wq.name, wq.state);
///     }
/// }
/// # Ok::<(), intel_dsa::DsaError>(())
/// ```
#[cfg(target_os = "linux")]
pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
    linux_impl::discover_devices()
}

#[cfg(target_os = "windows")]
pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
    windows_impl::discover_devices()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn discover_devices() -> Result<Vec<DsaDevice>, DsaError> {
    stub_impl::discover_devices()
}

/// Check if DSA is available on this system.
///
/// This performs a quick check without full device enumeration.
#[cfg(target_os = "linux")]
pub fn is_dsa_available() -> bool {
    linux_impl::is_dsa_available()
}

#[cfg(target_os = "windows")]
pub fn is_dsa_available() -> bool {
    windows_impl::is_dsa_available()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn is_dsa_available() -> bool {
    stub_impl::is_dsa_available()
}

/// Check if DSA devices are configured and ready to use.
#[cfg(target_os = "linux")]
pub fn is_dsa_configured() -> bool {
    linux_impl::is_dsa_configured()
}

#[cfg(target_os = "windows")]
pub fn is_dsa_configured() -> bool {
    windows_impl::is_dsa_configured()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn is_dsa_configured() -> bool {
    stub_impl::is_dsa_configured()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dsa_available() {
        // This test just verifies the function doesn't panic
        let _ = is_dsa_available();
    }

    #[test]
    fn test_is_dsa_configured() {
        // This test just verifies the function doesn't panic
        let _ = is_dsa_configured();
    }

    #[test]
    fn test_discover_on_non_dsa_system() {
        let result = discover_devices();
        match result {
            Ok(devices) => {
                println!("Found {} DSA devices", devices.len());
            }
            Err(DsaError::PlatformNotSupported) => {
                // Expected on systems without DSA support
            }
            Err(e) => {
                println!("Device discovery error: {}", e);
            }
        }
    }
}
