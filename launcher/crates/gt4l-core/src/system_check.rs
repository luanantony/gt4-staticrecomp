//! System requirements checked before setup and before Play (TASK-081): the processor and Vulkan.
//!
//! This module holds the rules only (pure functions over plain data), so every case is tested without the
//! hardware. `gt4l-platform::system` reads CPUID and probes the Vulkan loader and fills these structs.
//!
//! - **Processor.** SSE4.1 with its prerequisites SSE3 and SSSE3 (owner decision, 2026-10-21). `gt4vulkan.exe` is built
//!   for baseline x86-64 (`GT4_X86_64_V3` is off; its only AVX code is CPU-dispatched Visual C++ library code), and the
//!   PCSX2 GS (`gt4-gs.exe`) needs SSE4.1, PCSX2's published minimum. SSE state needs no OS opt-in on x64 Windows.
//!   `MINIMUM_CPU` is the single place to change the level.
//! - **Vulkan.** The game needs a GPU that reports Vulkan 1.2 or newer and the instance extensions `VK_KHR_surface` and
//!   `VK_KHR_win32_surface` (`native/native_vulkan.cpp`, `native/vulkan_extensions.cpp`). Swapchain maintenance 1 is
//!   used when present and is not required (slice214). The PCSX2 GS renders with Vulkan as well
//!   (`docs/GS-SETTINGS.md`, row `renderer`), so the check applies to both renderers.

/// What the processor reports (CPUID leaf 1, ECX).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuFeatures {
    /// CPUID.1:ECX bit 0.
    pub sse3: bool,
    /// CPUID.1:ECX bit 9.
    pub ssse3: bool,
    /// CPUID.1:ECX bit 19.
    pub sse4_1: bool,
}

/// The processor level the release requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuLevel {
    /// SSE4.1 with SSE3 and SSSE3 (Intel Core i3/i5/i7 and the late Core 2, AMD FX and newer).
    Sse41,
}

/// The release policy (see the module comment).
pub const MINIMUM_CPU: CpuLevel = CpuLevel::Sse41;

/// The message for a processor below the minimum (owner wording, TASK-081). SSE4.1 arrived with Intel's 45 nm Core 2
/// (2008) and AMD's FX/Bulldozer (2011); every Intel Core i and every AMD Ryzen has it.
pub const CPU_UNSUPPORTED_MESSAGE: &str = "Your processor is not supported: GT4Recomp needs a CPU with SSE4.1 \
(Intel Core i3/i5/i7 / AMD FX or newer).";

/// The hardware features missing for `level`, by name (empty when the processor has them all).
pub fn missing_cpu_features(features: &CpuFeatures, level: CpuLevel) -> Vec<&'static str> {
    match level {
        CpuLevel::Sse41 => {
            let required = [("SSE3", features.sse3), ("SSSE3", features.ssse3), ("SSE4.1", features.sse4_1)];
            required.iter().filter(|(_, present)| !present).map(|(name, _)| *name).collect()
        }
    }
}

/// `None` when the processor can run the release; otherwise the message for the player.
pub fn cpu_problem(features: &CpuFeatures) -> Option<String> {
    if missing_cpu_features(features, MINIMUM_CPU).is_empty() {
        None
    } else {
        Some(CPU_UNSUPPORTED_MESSAGE.to_string())
    }
}

/// The lowest Vulkan version the game accepts on its GPU (`native/native_vulkan.cpp`: VK_API_VERSION_1_2).
pub const MINIMUM_VULKAN: (u32, u32) = (1, 2);

/// The instance extensions `gt4vulkan.exe` requires (`native/vulkan_extensions.cpp`). The swapchain-maintenance ones
/// are optional since slice214.
pub const GAME_INSTANCE_EXTENSIONS: [&str; 2] = ["VK_KHR_surface", "VK_KHR_win32_surface"];

/// One GPU as the Vulkan loader reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VulkanDevice {
    pub name: String,
    pub api_major: u32,
    pub api_minor: u32,
    /// VkPhysicalDeviceProperties::vendorID (0x1002 AMD, 0x10DE NVIDIA, 0x8086 Intel, 0x1414 Microsoft).
    pub vendor_id: u32,
    /// VkPhysicalDeviceType: 0 other, 1 integrated, 2 discrete, 3 virtual, 4 CPU.
    pub device_type: u32,
}

/// Microsoft's vendor id: the "Microsoft Direct3D12 (...)" devices of the Vulkan-on-D3D12 layer (Dozen, part of the
/// OpenCL, OpenGL and Vulkan Compatibility Pack). They are a translation layer, not the GPU's own Vulkan driver.
pub const MICROSOFT_VENDOR_ID: u32 = 0x1414;
/// VK_PHYSICAL_DEVICE_TYPE_CPU: a software rasterizer (llvmpipe, SwiftShader).
pub const DEVICE_TYPE_CPU: u32 = 4;
/// VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU: a GPU on the processor die (Intel UHD, AMD Radeon graphics).
pub const DEVICE_TYPE_INTEGRATED: u32 = 1;

/// True for a GPU with its own Vulkan driver: not a software rasterizer and not the D3D12 translation layer.
pub fn is_gpu_driver(device: &VulkanDevice) -> bool {
    device.vendor_id != MICROSOFT_VENDOR_ID && device.device_type != DEVICE_TYPE_CPU
}

/// The name of the integrated GPU the game will render on, or `None` when the lighter default does not apply (issue
/// #89). The game's GSdx picks its adapter itself and the launcher cannot tell which one it takes, so the rule is
/// conservative: the integrated default applies only when every GPU with its own driver is integrated. With a discrete
/// card in the machine (or none at all) the answer is `None`, which keeps today's default. The first integrated
/// device's name is returned for the log.
pub fn integrated_gpu_name(devices: &[VulkanDevice]) -> Option<String> {
    let drivers: Vec<&VulkanDevice> = devices.iter().filter(|device| is_gpu_driver(device)).collect();
    if drivers.is_empty() || drivers.iter().any(|device| device.device_type != DEVICE_TYPE_INTEGRATED) {
        return None;
    }
    drivers.first().map(|device| device.name.clone())
}

/// What the Vulkan probe found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VulkanProbe {
    /// `vulkan-1.dll` is not in System32 or does not export `vkGetInstanceProcAddr`.
    LoaderMissing,
    /// The loader is present, but `vkCreateInstance` failed (VkResult).
    InstanceFailed { code: i32 },
    /// The loader works. `instance_extensions` lists what the loader and drivers offer.
    Ready { instance_extensions: Vec<String>, devices: Vec<VulkanDevice> },
}

const DRIVER_ADVICE: &str = "Update your graphics driver from NVIDIA, AMD or Intel, then start GT4Recomp again.";

/// `None` when the game can create its Vulkan instance and a GPU reports at least `MINIMUM_VULKAN`; otherwise the
/// message for the player.
pub fn vulkan_problem(probe: &VulkanProbe) -> Option<String> {
    let (major, minor) = MINIMUM_VULKAN;
    match probe {
        VulkanProbe::LoaderMissing => Some(format!(
            "Vulkan is not installed on this PC. GT4Recomp needs a graphics card with Vulkan {major}.{minor} or newer. \
             {DRIVER_ADVICE}"
        )),
        VulkanProbe::InstanceFailed { code } => Some(format!(
            "Vulkan did not start on this PC (error {code}). GT4Recomp needs a graphics card with Vulkan \
             {major}.{minor} or newer. {DRIVER_ADVICE}"
        )),
        VulkanProbe::Ready { instance_extensions, devices } => {
            let devices: Vec<&VulkanDevice> = devices.iter().filter(|device| is_gpu_driver(device)).collect();
            if devices.is_empty() {
                return Some(format!(
                    "No graphics card with Vulkan was found. GT4Recomp needs a graphics card with Vulkan \
                     {major}.{minor} or newer. {DRIVER_ADVICE}"
                ));
            }
            if !devices.iter().any(|device| (device.api_major, device.api_minor) >= (major, minor)) {
                let best = devices
                    .iter()
                    .max_by_key(|device| (device.api_major, device.api_minor))
                    .expect("devices is not empty");
                return Some(format!(
                    "Your graphics card ({}) reports Vulkan {}.{}; GT4Recomp needs Vulkan {major}.{minor} or newer. \
                     {DRIVER_ADVICE}",
                    best.name, best.api_major, best.api_minor
                ));
            }
            let missing = missing_instance_extensions(instance_extensions);
            if !missing.is_empty() {
                return Some(format!(
                    "Your graphics driver is too old for GT4Recomp (missing Vulkan {}). {DRIVER_ADVICE}",
                    missing.join(", ")
                ));
            }
            None
        }
    }
}

/// The game's required instance extensions that the loader does not offer.
pub fn missing_instance_extensions(offered: &[String]) -> Vec<&'static str> {
    GAME_INSTANCE_EXTENSIONS.iter().copied().filter(|name| !offered.iter().any(|offer| offer == name)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn penryn() -> CpuFeatures {
        CpuFeatures { sse3: true, ssse3: true, sse4_1: true }
    }

    #[test]
    fn an_sse4_1_processor_passes() {
        assert_eq!(cpu_problem(&penryn()), None);
        assert!(missing_cpu_features(&penryn(), CpuLevel::Sse41).is_empty());
    }

    #[test]
    fn each_missing_feature_is_refused_with_the_owner_message() {
        let cases: [(&str, fn(&mut CpuFeatures)); 3] =
            [("SSE3", |f| f.sse3 = false), ("SSSE3", |f| f.ssse3 = false), ("SSE4.1", |f| f.sse4_1 = false)];
        for (name, clear) in cases {
            let mut features = penryn();
            clear(&mut features);
            assert_eq!(missing_cpu_features(&features, CpuLevel::Sse41), vec![name]);
            assert_eq!(cpu_problem(&features).as_deref(), Some(CPU_UNSUPPORTED_MESSAGE), "{name}");
        }
    }

    #[test]
    fn conroe_core_2_and_phenom_ii_are_refused() {
        // 65 nm Core 2 (Conroe/Merom): SSE3 and SSSE3, no SSE4.1. Phenom II: SSE3 (and SSE4a), no SSSE3 or SSE4.1.
        let conroe = CpuFeatures { sse4_1: false, ..penryn() };
        let phenom = CpuFeatures { sse3: true, ssse3: false, sse4_1: false };
        assert_eq!(missing_cpu_features(&conroe, CpuLevel::Sse41), vec!["SSE4.1"]);
        assert_eq!(missing_cpu_features(&phenom, CpuLevel::Sse41), vec!["SSSE3", "SSE4.1"]);
        assert_eq!(cpu_problem(&conroe).as_deref(), Some(CPU_UNSUPPORTED_MESSAGE));
        assert_eq!(cpu_problem(&phenom).as_deref(), Some(CPU_UNSUPPORTED_MESSAGE));
    }

    #[test]
    fn nothing_detected_is_the_unsupported_message() {
        assert_eq!(cpu_problem(&CpuFeatures::default()).as_deref(), Some(CPU_UNSUPPORTED_MESSAGE));
        assert!(CPU_UNSUPPORTED_MESSAGE.contains("SSE4.1"));
    }

    fn required_extensions() -> Vec<String> {
        GAME_INSTANCE_EXTENSIONS.iter().map(|name| name.to_string()).collect()
    }

    fn device(name: &str, major: u32, minor: u32) -> VulkanDevice {
        VulkanDevice { name: name.into(), api_major: major, api_minor: minor, vendor_id: 0x1002, device_type: 2 }
    }

    #[test]
    fn a_vulkan_1_2_gpu_with_only_the_surface_extensions_passes() {
        // An older driver: no get_surface_capabilities2, no surface_maintenance1 (the game uses its baseline path).
        let probe = VulkanProbe::Ready { instance_extensions: required_extensions(), devices: vec![device("GPU", 1, 2)] };
        assert_eq!(vulkan_problem(&probe), None);
        let mut newer = required_extensions();
        newer.extend(["VK_KHR_get_surface_capabilities2".to_string(), "VK_KHR_surface_maintenance1".to_string()]);
        let probe = VulkanProbe::Ready { instance_extensions: newer, devices: vec![device("GPU", 1, 4)] };
        assert_eq!(vulkan_problem(&probe), None);
    }

    #[test]
    fn one_good_gpu_among_old_ones_passes() {
        let probe = VulkanProbe::Ready {
            instance_extensions: required_extensions(),
            devices: vec![device("Old", 1, 0), device("New", 1, 3)],
        };
        assert_eq!(vulkan_problem(&probe), None);
    }

    #[test]
    fn no_loader_no_instance_no_gpu_and_old_gpu_name_the_driver_update() {
        let probes = [
            VulkanProbe::LoaderMissing,
            VulkanProbe::InstanceFailed { code: -9 },
            VulkanProbe::Ready { instance_extensions: required_extensions(), devices: vec![] },
            VulkanProbe::Ready { instance_extensions: required_extensions(), devices: vec![device("HD 4000", 1, 0)] },
        ];
        for probe in probes {
            let message = vulkan_problem(&probe).expect("refused");
            assert!(message.contains("Update your graphics driver"), "{message}");
            assert!(message.contains("Vulkan"), "{message}");
        }
        let old =
            VulkanProbe::Ready { instance_extensions: required_extensions(), devices: vec![device("HD 4000", 1, 1)] };
        assert!(vulkan_problem(&old).unwrap().contains("(HD 4000) reports Vulkan 1.1"));
    }

    #[test]
    fn the_d3d12_translation_layer_and_software_rasterizers_do_not_count() {
        let dozen = VulkanDevice {
            name: "Microsoft Direct3D12 (Microsoft Basic Render Driver)".into(),
            api_major: 1,
            api_minor: 2,
            vendor_id: MICROSOFT_VENDOR_ID,
            device_type: 1,
        };
        let llvmpipe =
            VulkanDevice { name: "llvmpipe".into(), api_major: 1, api_minor: 3, vendor_id: 0x10005, device_type: 4 };
        assert!(!is_gpu_driver(&dozen) && !is_gpu_driver(&llvmpipe));
        let only_layers =
            VulkanProbe::Ready { instance_extensions: required_extensions(), devices: vec![dozen.clone(), llvmpipe] };
        assert!(vulkan_problem(&only_layers).unwrap().contains("No graphics card with Vulkan was found"));
        // This machine's real list (2026-10-21): the AMD driver plus two Dozen devices.
        let real = VulkanProbe::Ready {
            instance_extensions: required_extensions(),
            devices: vec![device("AMD Radeon RX 9060 XT", 1, 4), dozen],
        };
        assert_eq!(vulkan_problem(&real), None);
    }

    #[test]
    fn a_loader_without_the_win32_surface_is_refused_by_name() {
        let offered = vec!["VK_KHR_surface".to_string()];
        let probe = VulkanProbe::Ready { instance_extensions: offered.clone(), devices: vec![device("GPU", 1, 3)] };
        let message = vulkan_problem(&probe).expect("refused");
        assert!(message.contains("missing Vulkan VK_KHR_win32_surface"), "{message}");
        assert_eq!(missing_instance_extensions(&offered), vec!["VK_KHR_win32_surface"]);
    }

    #[test]
    fn the_required_list_matches_the_game_source() {
        // native/vulkan_extensions.cpp requires exactly these two; the maintenance extensions are optional there.
        let source = include_str!("../../../../native/vulkan_extensions.cpp");
        let start = source.find("for (const char *required : {").expect("required list in vulkan_extensions.cpp");
        let block = &source[start..start + source[start..].find('}').expect("end of list")];
        for name in GAME_INSTANCE_EXTENSIONS {
            assert!(block.contains(&format!("\"{name}\"")), "{name} not in the game's required list");
        }
        assert_eq!(block.matches("\"VK_").count(), GAME_INSTANCE_EXTENSIONS.len());
        let vulkan = include_str!("../../../../native/native_vulkan.cpp");
        assert!(vulkan.contains("properties.apiVersion < VK_API_VERSION_1_2"), "the game's minimum is Vulkan 1.2");
    }

    fn integrated(name: &str) -> VulkanDevice {
        VulkanDevice { name: name.into(), api_major: 1, api_minor: 3, vendor_id: 0x8086, device_type: 1 }
    }

    #[test]
    fn an_integrated_only_machine_is_integrated_with_its_first_name() {
        let devices = vec![integrated("Intel(R) UHD Graphics (Ice Lake)")];
        assert_eq!(integrated_gpu_name(&devices).as_deref(), Some("Intel(R) UHD Graphics (Ice Lake)"));
    }

    #[test]
    fn a_discrete_card_among_integrated_ones_keeps_the_discrete_default() {
        let laptop = vec![integrated("Intel UHD"), device("NVIDIA RTX", 1, 3)];
        assert_eq!(integrated_gpu_name(&laptop), None);
        let reversed = vec![device("AMD Radeon RX 9060 XT", 1, 4), integrated("Intel UHD")];
        assert_eq!(integrated_gpu_name(&reversed), None);
    }

    #[test]
    fn no_driver_gpu_is_not_integrated() {
        assert_eq!(integrated_gpu_name(&[]), None);
        let dozen = VulkanDevice {
            name: "Microsoft Direct3D12 (Intel UHD)".into(),
            api_major: 1,
            api_minor: 2,
            vendor_id: MICROSOFT_VENDOR_ID,
            device_type: 1,
        };
        let llvmpipe =
            VulkanDevice { name: "llvmpipe".into(), api_major: 1, api_minor: 3, vendor_id: 0x10005, device_type: 4 };
        assert_eq!(integrated_gpu_name(&[dozen, llvmpipe]), None);
    }
}
