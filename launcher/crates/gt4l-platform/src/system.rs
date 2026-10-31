//! Reads the processor features and probes Vulkan for the requirement checks (TASK-081). The rules that turn these
//! readings into a message are pure and tested in `gt4l_core::system_check`.
//!
//! - `read_cpu_features`: CPUID leaf 1 (SSE3, SSSE3, SSE4.1).
//! - `probe_vulkan`: loads `vulkan-1.dll` from System32 (as the game does), lists the instance extensions, creates a
//!   bare Vulkan 1.0 instance with no extensions or layers, lists the GPUs with their Vulkan version, and destroys the
//!   instance. It opens no window and allocates no GPU memory. A loaded driver stays in the launcher process until it
//!   exits; the probe runs once per launch when it succeeds (`checked_vulkan`).

use std::sync::{Mutex, OnceLock};

use gt4l_core::system_check::{CpuFeatures, VulkanDevice, VulkanProbe};

/// The processor features the rules need (CPUID leaf 1, ECX: SSE3, SSSE3, SSE4.1).
#[cfg(target_arch = "x86_64")]
pub fn read_cpu_features() -> CpuFeatures {
    use std::arch::x86_64::__cpuid;

    let bit = |value: u32, index: u32| value & (1 << index) != 0;
    // CPUID is always present on x86-64.
    #[allow(unused_unsafe)]
    let leaf1 = unsafe { __cpuid(1) };
    CpuFeatures { sse3: bit(leaf1.ecx, 0), ssse3: bit(leaf1.ecx, 9), sse4_1: bit(leaf1.ecx, 19) }
}

/// Other targets are not supported by the game; report nothing so the rules refuse.
#[cfg(not(target_arch = "x86_64"))]
pub fn read_cpu_features() -> CpuFeatures {
    CpuFeatures::default()
}

/// The integrated GPU the game renders on, read once per launcher process (issue #89): its name, or `None` when the
/// machine has a discrete card, no GPU with its own driver, or the probe fails. The rule is
/// `gt4l_core::system_check::integrated_gpu_name`; the same probe as the system check.
static INTEGRATED_GPU: OnceLock<Option<String>> = OnceLock::new();

pub fn integrated_gpu_name() -> Option<String> {
    INTEGRATED_GPU
        .get_or_init(|| match probe_vulkan() {
            VulkanProbe::Ready { devices, .. } => gt4l_core::system_check::integrated_gpu_name(&devices),
            _ => None,
        })
        .clone()
}

/// The last successful Vulkan check of this launcher session. A failure is not kept, so a driver installed while the
/// launcher is open is seen at the next Play.
static VULKAN_OK: Mutex<bool> = Mutex::new(false);

/// `None` when Vulkan is fine (probed once per session after the first success), else the player's message.
pub fn checked_vulkan() -> Option<String> {
    let mut ok = VULKAN_OK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if *ok {
        return None;
    }
    let problem = gt4l_core::system_check::vulkan_problem(&probe_vulkan());
    *ok = problem.is_none();
    problem
}

/// The CPU check, then the Vulkan check: the first problem's message, or `None`.
pub fn requirements_problem() -> Option<String> {
    gt4l_core::system_check::cpu_problem(&read_cpu_features()).or_else(checked_vulkan)
}

#[cfg(windows)]
pub fn probe_vulkan() -> VulkanProbe {
    vulkan_ffi::probe()
}

#[cfg(not(windows))]
pub fn probe_vulkan() -> VulkanProbe {
    VulkanProbe::LoaderMissing
}

/// The few Vulkan 1.0 entry points the probe uses, declared by hand (no Vulkan crate in the launcher).
#[cfg(windows)]
mod vulkan_ffi {
    use std::ffi::{c_char, c_void, CStr};
    use std::ptr;

    use windows::core::{s, w};
    use windows::Win32::Foundation::FreeLibrary;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};

    use super::{VulkanDevice, VulkanProbe};

    type VkInstance = *mut c_void;
    type VkPhysicalDevice = *mut c_void;
    type VkResult = i32;
    const VK_SUCCESS: VkResult = 0;
    const VK_INCOMPLETE: VkResult = 5;
    const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
    const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
    const VK_API_VERSION_1_0: u32 = 1 << 22;
    const VK_MAX_EXTENSION_NAME_SIZE: usize = 256;

    #[repr(C)]
    struct VkApplicationInfo {
        s_type: u32,
        p_next: *const c_void,
        p_application_name: *const c_char,
        application_version: u32,
        p_engine_name: *const c_char,
        engine_version: u32,
        api_version: u32,
    }

    #[repr(C)]
    struct VkInstanceCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        p_application_info: *const VkApplicationInfo,
        enabled_layer_count: u32,
        pp_enabled_layer_names: *const *const c_char,
        enabled_extension_count: u32,
        pp_enabled_extension_names: *const *const c_char,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VkExtensionProperties {
        extension_name: [c_char; VK_MAX_EXTENSION_NAME_SIZE],
        spec_version: u32,
    }

    /// VkPhysicalDeviceProperties starts with these fields; the rest (limits, sparse properties) is not read. The
    /// probe hands the driver a larger zeroed buffer (`PROPERTIES_BUFFER`) and reads only this prefix.
    #[repr(C)]
    struct PropertiesPrefix {
        api_version: u32,
        driver_version: u32,
        vendor_id: u32,
        device_id: u32,
        device_type: u32,
        device_name: [c_char; 256],
    }
    /// sizeof(VkPhysicalDeviceProperties) is 824 bytes on x64; 4 KiB leaves room for any driver.
    const PROPERTIES_BUFFER: usize = 4096;

    type GetInstanceProcAddr = unsafe extern "system" fn(VkInstance, *const c_char) -> Option<unsafe extern "system" fn()>;
    type EnumerateInstanceExtensionProperties =
        unsafe extern "system" fn(*const c_char, *mut u32, *mut VkExtensionProperties) -> VkResult;
    type CreateInstance =
        unsafe extern "system" fn(*const VkInstanceCreateInfo, *const c_void, *mut VkInstance) -> VkResult;
    type DestroyInstance = unsafe extern "system" fn(VkInstance, *const c_void);
    type EnumeratePhysicalDevices = unsafe extern "system" fn(VkInstance, *mut u32, *mut VkPhysicalDevice) -> VkResult;
    type GetPhysicalDeviceProperties = unsafe extern "system" fn(VkPhysicalDevice, *mut c_void);

    fn text(raw: &[c_char]) -> String {
        let bytes: Vec<u8> = raw.iter().take_while(|byte| **byte != 0).map(|byte| *byte as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    pub fn probe() -> VulkanProbe {
        // SAFETY: the library is loaded from System32 only and freed at the end; every function pointer comes from the
        // loader's own vkGetInstanceProcAddr and is called with the Vulkan 1.0 signatures declared above.
        unsafe {
            let Ok(library) = LoadLibraryExW(w!("vulkan-1.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) else {
                return VulkanProbe::LoaderMissing;
            };
            let result = match GetProcAddress(library, s!("vkGetInstanceProcAddr")) {
                Some(address) => {
                    let get: GetInstanceProcAddr = std::mem::transmute(address);
                    probe_with(get)
                }
                None => VulkanProbe::LoaderMissing,
            };
            let _ = FreeLibrary(library);
            result
        }
    }

    unsafe fn load<T>(get: GetInstanceProcAddr, instance: VkInstance, name: &CStr) -> Option<T> {
        let address = get(instance, name.as_ptr())?;
        Some(std::mem::transmute_copy::<unsafe extern "system" fn(), T>(&address))
    }

    unsafe fn probe_with(get: GetInstanceProcAddr) -> VulkanProbe {
        let null = ptr::null_mut();
        let (Some(enumerate_extensions), Some(create_instance)) = (
            load::<EnumerateInstanceExtensionProperties>(get, null, c"vkEnumerateInstanceExtensionProperties"),
            load::<CreateInstance>(get, null, c"vkCreateInstance"),
        ) else {
            return VulkanProbe::LoaderMissing;
        };

        let mut instance_extensions = Vec::new();
        let mut count = 0u32;
        if enumerate_extensions(ptr::null(), &mut count, ptr::null_mut()) == VK_SUCCESS && count > 0 {
            let empty = VkExtensionProperties { extension_name: [0; VK_MAX_EXTENSION_NAME_SIZE], spec_version: 0 };
            let mut list = vec![empty; count as usize];
            let code = enumerate_extensions(ptr::null(), &mut count, list.as_mut_ptr());
            if code == VK_SUCCESS || code == VK_INCOMPLETE {
                list.truncate(count as usize);
                instance_extensions = list.iter().map(|item| text(&item.extension_name)).collect();
            }
        }

        let application = VkApplicationInfo {
            s_type: VK_STRUCTURE_TYPE_APPLICATION_INFO,
            p_next: ptr::null(),
            p_application_name: c"GT4Recomp launcher check".as_ptr(),
            application_version: 0,
            p_engine_name: ptr::null(),
            engine_version: 0,
            api_version: VK_API_VERSION_1_0,
        };
        let create_info = VkInstanceCreateInfo {
            s_type: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
            p_next: ptr::null(),
            flags: 0,
            p_application_info: &application,
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: 0,
            pp_enabled_extension_names: ptr::null(),
        };
        let mut instance: VkInstance = ptr::null_mut();
        let code = create_instance(&create_info, ptr::null(), &mut instance);
        if code != VK_SUCCESS || instance.is_null() {
            return VulkanProbe::InstanceFailed { code };
        }

        let mut devices = Vec::new();
        let destroy = load::<DestroyInstance>(get, instance, c"vkDestroyInstance");
        let enumerate_devices = load::<EnumeratePhysicalDevices>(get, instance, c"vkEnumeratePhysicalDevices");
        let properties_of = load::<GetPhysicalDeviceProperties>(get, instance, c"vkGetPhysicalDeviceProperties");
        if let (Some(enumerate_devices), Some(properties_of)) = (enumerate_devices, properties_of) {
            let mut count = 0u32;
            if enumerate_devices(instance, &mut count, ptr::null_mut()) == VK_SUCCESS && count > 0 {
                let mut handles: Vec<VkPhysicalDevice> = vec![ptr::null_mut(); count as usize];
                let code = enumerate_devices(instance, &mut count, handles.as_mut_ptr());
                if code == VK_SUCCESS || code == VK_INCOMPLETE {
                    handles.truncate(count as usize);
                    for handle in handles {
                        // u64 storage keeps the buffer 8-byte aligned, as the driver's struct needs.
                        let mut buffer = vec![0u64; PROPERTIES_BUFFER / 8];
                        properties_of(handle, buffer.as_mut_ptr().cast());
                        let prefix = &*(buffer.as_ptr() as *const PropertiesPrefix);
                        devices.push(VulkanDevice {
                            name: text(&prefix.device_name),
                            api_major: (prefix.api_version >> 22) & 0x7F,
                            api_minor: (prefix.api_version >> 12) & 0x3FF,
                            vendor_id: prefix.vendor_id,
                            device_type: prefix.device_type,
                        });
                    }
                }
            }
        }
        if let Some(destroy) = destroy {
            destroy(instance, ptr::null());
        }
        VulkanProbe::Ready { instance_extensions, devices }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads this machine; the rules are tested in gt4l-core. Here only the plumbing: the bits are consistent.
    #[test]
    fn cpu_reading_is_consistent() {
        let features = read_cpu_features();
        // Every x86-64 CPU that has SSE4.1 also has SSSE3 and SSE3.
        if features.sse4_1 {
            assert!(features.ssse3 && features.sse3);
        }
    }

    /// Prints what this machine's Vulkan loader reports. Ignored by default: it loads the GPU driver.
    #[test]
    #[ignore]
    fn vulkan_probe_on_this_machine() {
        let probe = probe_vulkan();
        println!("{probe:#?}");
        println!("problem: {:?}", gt4l_core::system_check::vulkan_problem(&probe));
        println!("cpu: {:?} problem: {:?}", read_cpu_features(), gt4l_core::system_check::cpu_problem(&read_cpu_features()));
    }
}
