use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use regex::Regex;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::symlink_file;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use winreg::enums::*;
use winreg::RegKey;

// --- CONFIGURATION (FIXED URLS) ---
const ZLUDA_URL: &str = "https://github.com/lshqqytiger/ZLUDA/releases/download/v6/ZLUDA-windows-amd64.zip";
const DOOM_URL: &str = "https://github.com/chocolate-doom/chocolate-doom/releases/download/chocolate-doom-3.0.1/chocolate-doom-3.0.1-win32.zip";
const WAD_URL: &str = "https://distro.ibiblio.org/slitaz/sources/packages/d/doom1.wad";
const MUSIC_URL: &str = "https://www.soundhelix.com/examples/mp3/SoundHelix-Song-4.mp3";
const HIPIFY_URL: &str = "https://raw.githubusercontent.com/ROCm/HIPIFY/rocm-6.1.0/bin/hipify-perl";
const PERL_URL: &str = "https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/download/SP_54221_64bit/strawberry-perl-5.42.2.1-64bit-portable.zip";
const AMD_HIP_SDK_URL: &str = "https://download.amd.com/developer/eula/rocm-hub/AMD-Software-PRO-Edition-24.Q3-Win10-Win11-For-HIP.exe";

static MUSIC_PLAYING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct GhostPaths {
    _base: PathBuf,
    zluda: PathBuf,
    doom: PathBuf,
    music: PathBuf,
    spoof: PathBuf,
    hipify: PathBuf,
    perl: PathBuf,
}

impl GhostPaths {
    fn new() -> Self {
        let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
        let base = Path::new(&user_profile).join(".ghost");
        GhostPaths {
            zluda: base.join("zluda"),
            doom: base.join("doom"),
            music: base.join("music"),
            spoof: base.join("nv_spoof"),
            hipify: base.join("hipify"),
            perl: base.join("perl"),
            _base: base,
        }
    }
    fn init(&self) {
        let _ = fs::create_dir_all(&self.zluda);
        let _ = fs::create_dir_all(&self.doom);
        let _ = fs::create_dir_all(&self.music);
        let _ = fs::create_dir_all(&self.spoof);
        let _ = fs::create_dir_all(&self.hipify);
        let _ = fs::create_dir_all(&self.perl);
    }
}

struct RegistryGuard { path: String }
impl RegistryGuard {
    fn new() -> Self {
        let path = "SOFTWARE\\NVIDIA Corporation";
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if hklm.create_subkey(path).is_ok() { return Self { path: path.to_string() }; }
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.create_subkey(path);
        Self { path: path.to_string() }
    }
}
impl Drop for RegistryGuard {
    fn drop(&mut self) {
        let _ = RegKey::predef(HKEY_LOCAL_MACHINE).delete_subkey_all(&self.path);
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.path);
    }
}

fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_file_recursive(&path, filename) {
                    return Some(found);
                }
            } else if path.file_name().unwrap_or_default() == filename {
                return Some(path);
            }
        }
    }
    None
}

fn download_file_with_progress(url: &str, dest: &Path, msg: &str) -> Result<(), Box<dyn std::error::Error>> {
    if dest.exists() { return Ok(()); }
    
    let mut stdout = io::stdout();
    let _ = execute!(stdout, SetForegroundColor(Color::Yellow));
    println!("{}", msg);
    let _ = execute!(stdout, ResetColor);

    let client = reqwest::blocking::Client::builder().user_agent("GhostEnv/9.1").build().unwrap();
    let reqwest_success = match client.get(url).send() {
        Ok(mut response) if response.status().is_success() => {
            match File::create(dest) {
                Ok(mut file) => response.copy_to(&mut file).is_ok(),
                Err(_) => false,
            }
        }
        _ => false,
    };

    if reqwest_success && dest.exists() {
        let _ = execute!(stdout, SetForegroundColor(Color::Green));
        println!("  -> Download Complete (reqwest)!");
        let _ = execute!(stdout, ResetColor);
        return Ok(());
    }

    let _ = fs::remove_file(dest);
    let _ = execute!(stdout, SetForegroundColor(Color::Yellow));
    println!("  -> reqwest failed, falling back to PowerShell...");
    let _ = execute!(stdout, ResetColor);

    let ps_cmd = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
        url, dest.display()
    );
    
    let status = Command::new("powershell")
        .args(&["-NoProfile", "-Command", &ps_cmd])
        .status();

    if status.map_or(false, |s| s.success()) && dest.exists() {
        let _ = execute!(stdout, SetForegroundColor(Color::Green));
        println!("  -> Download Complete (PowerShell)!");
        let _ = execute!(stdout, ResetColor);
        return Ok(());
    }

    let _ = execute!(stdout, SetForegroundColor(Color::Red));
    println!("  -> ERROR: Download failed for {}", url);
    let _ = execute!(stdout, ResetColor);
    Err("Download failed.".into())
}

fn extract_and_flatten(archive_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    archive.extract(dest_dir)?;
    
    if let Ok(entries) = fs::read_dir(dest_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                    for sub in sub_entries.flatten() {
                        let _ = fs::rename(sub.path(), dest_dir.join(sub.file_name()));
                    }
                }
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    Ok(())
}

fn get_zluda_url() -> String {
    let client = reqwest::blocking::Client::builder().user_agent("GhostEnv/9.1").build().unwrap();
    if let Ok(res) = client.get("https://api.github.com/repos/vosen/zluda/releases").send() {
        if let Ok(text) = res.text() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(releases) = json.as_array() {
                    for release in releases {
                        if let Some(assets) = release["assets"].as_array() {
                            for asset in assets {
                                if let Some(name) = asset["name"].as_str() {
                                    if name.contains("windows") && name.ends_with(".zip") {
                                        return asset["browser_download_url"].as_str().unwrap().to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    ZLUDA_URL.to_string()
}

fn find_cl_exe() -> bool {
    if Command::new("cl.exe").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() { return true; }
    let vswhere = Path::new("C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe");
    if vswhere.exists() {
        if let Ok(output) = Command::new(vswhere).args(&["-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"]).output() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let vs_path = Path::new(&path_str);
                if let Ok(entries) = fs::read_dir(vs_path.join("VC\\Tools\\MSVC")) {
                    for entry in entries.flatten() {
                        let bin_path = entry.path().join("bin\\Hostx64\\x64");
                        if bin_path.join("cl.exe").exists() {
                            let current_path = env::var("PATH").unwrap_or_default();
                            unsafe { env::set_var("PATH", format!("{};{}", bin_path.display(), current_path)); }
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn ensure_dependencies(paths: &GhostPaths) {
    // FIX: Re-initialize the directories in case the user ran the 'clean' command!
    paths.init(); 

    let mut stdout = io::stdout();
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║             GHOST DEPENDENCY BOOTSTRAPPER                ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");
    let _ = execute!(stdout, ResetColor);

    if find_file_recursive(&paths.zluda, "nvcuda.dll").is_none() && find_file_recursive(&paths.zluda, "zluda.exe").is_none() {
        let zluda_url = get_zluda_url();
        let zip_path = paths.zluda.join("zluda.zip");
        if download_file_with_progress(&zluda_url, &zip_path, "[1/7] Downloading ZLUDA Engine...").is_ok() {
            let _ = extract_and_flatten(&zip_path, &paths.zluda);
            let _ = fs::remove_file(zip_path);
            
            if let Some(exe_path) = find_file_recursive(&paths.zluda, "zluda.exe") {
                let _ = fs::copy(&exe_path, paths.zluda.join("zluda.exe"));
            }
            if let Some(dll_path) = find_file_recursive(&paths.zluda, "nvcuda.dll") {
                let _ = fs::copy(&dll_path, paths.zluda.join("nvcuda.dll"));
            }
        }
    }

    if find_file_recursive(&paths.doom, "chocolate-doom.exe").is_none() {
        let zip_path = paths.doom.join("doom.zip");
        if download_file_with_progress(DOOM_URL, &zip_path, "[2/7] Downloading DOOM Engine...").is_ok() {
            let _ = extract_and_flatten(&zip_path, &paths.doom);
            let _ = fs::remove_file(zip_path);
            
            if let Some(exe_path) = find_file_recursive(&paths.doom, "chocolate-doom.exe") {
                let _ = fs::copy(&exe_path, paths.doom.join("chocolate-doom.exe"));
            }
        }
    }
    let wad_file = paths.doom.join("DOOM1.WAD");
    if !wad_file.exists() {
        let _ = download_file_with_progress(WAD_URL, &wad_file, "[3/7] Downloading DOOM WAD...");
    }

    let track = paths.music.join("Ghost_Track_4.mp3");
    if !track.exists() {
        let _ = download_file_with_progress(MUSIC_URL, &track, "[4/7] Downloading Music...");
    }

    let hipify_script = paths.hipify.join("hipify-perl");
    if !hipify_script.exists() {
        let _ = download_file_with_progress(HIPIFY_URL, &hipify_script, "[5/7] Downloading HIPIFY...");
    }

    if Command::new("perl").arg("-v").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
        if find_file_recursive(&paths.perl, "perl.exe").is_none() {
            let zip_path = paths.perl.join("perl.zip");
            if download_file_with_progress(PERL_URL, &zip_path, "[6/7] Downloading Strawberry Perl...").is_ok() {
                let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
                println!("  -> Extracting Strawberry Perl (This may take a minute)...");
                let _ = execute!(stdout, ResetColor);
                
                let ps_cmd = format!("Expand-Archive -Path '{}' -DestinationPath '{}' -Force", zip_path.display(), paths.perl.display());
                if Command::new("powershell").args(&["-NoProfile", "-Command", &ps_cmd]).status().map_or(false, |s| s.success()) {
                    let _ = fs::remove_file(zip_path);
                }
            }
        }
        
        if let Some(perl_exe) = find_file_recursive(&paths.perl, "perl.exe") {
            let perl_bin = perl_exe.parent().unwrap();
            let current_path = env::var("PATH").unwrap_or_default();
            if !current_path.contains(perl_bin.to_str().unwrap()) {
                unsafe { env::set_var("PATH", format!("{};{}", perl_bin.display(), current_path)); }
            }
        }
    }

    let hip_sdk_exe = paths._base.join("AMD_HIP_SDK.exe");
    if !hip_sdk_exe.exists() {
        if download_file_with_progress(AMD_HIP_SDK_URL, &hip_sdk_exe, "[7/7] Downloading AMD HIP SDK...").is_ok() {
            let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
            println!("  -> Launching AMD HIP SDK Installer...");
            let _ = execute!(stdout, ResetColor);
            
            Command::new(&hip_sdk_exe).spawn().ok();
        }
    }

    println!("\n[GHOST] Bootstrapping complete.\n");
}

struct GpuInfo {
    name: String,
    vram_bytes: u64,
    os_index: u64,
}

fn get_amd_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();
    let ps_script = r#"
    $gpus = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'AMD|Radeon|Advanced Micro Devices|Instinct' }
    $res = @()
    $rawIndex = 0
    foreach ($g in $gpus) {
        $vram = $g.AdapterRAM
        $pnp = Get-CimInstance Win32_PnPEntity -Filter "DeviceID='$($g.PNPDeviceID -replace '\\', '\\')'" -ErrorAction SilentlyContinue
        if ($pnp) {
            $drv = $pnp.GetCimSession().QueryInstances("root\cimv2", "WQL", "SELECT * FROM Win32_PnPSignedDriver WHERE DeviceID='$($pnp.DeviceID -replace '\\', '\\')'") | Select-Object -ExpandProperty DriverName -ErrorAction SilentlyContinue
            if ($drv) {
                $reg = Get-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\Class\$drv" -Name "HardwareInformation.qwMemorySize" -ErrorAction SilentlyContinue
                if ($reg -and $null -ne $reg."HardwareInformation.qwMemorySize") {
                    $vram = $reg."HardwareInformation.qwMemorySize"
                }
            }
        }
        $is_igpu = ($g.Name -match "Graphics" -and $g.Name -notmatch "RX|PRO|Instinct")
        if ([uint64]$vram -ge 2147483648 -and -not $is_igpu) {
            $res += [PSCustomObject]@{ Name = $g.Name; VRAM = [uint64]$vram; OsIndex = $rawIndex }
        }
        $rawIndex++
    }
    $res | ConvertTo-Json -Compress
    "#;

    if let Ok(output) = Command::new("powershell").args(&["-NoProfile", "-Command", ps_script]).output() {
        let json_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(arr) = parsed.as_array() {
                for item in arr {
                    if let (Some(name), Some(vram), Some(idx)) = (item["Name"].as_str(), item["VRAM"].as_u64(), item["OsIndex"].as_u64()) {
                        gpus.push(GpuInfo { name: name.to_string(), vram_bytes: vram, os_index: idx });
                    }
                }
            } else if let (Some(name), Some(vram), Some(idx)) = (parsed["Name"].as_str(), parsed["VRAM"].as_u64(), parsed["OsIndex"].as_u64()) {
                gpus.push(GpuInfo { name: name.to_string(), vram_bytes: vram, os_index: idx });
            }
        }
    }
    gpus.sort_by(|a, b| b.vram_bytes.cmp(&a.vram_bytes));
    gpus.into_iter().take(2).collect()
}

fn get_mapping(gpu_name: &str) -> (&'static str, &'static str, &'static str) {
    let name = gpu_name.to_lowercase();
    let mut series = "DEFAULT";
    if name.contains("rx 9") || name.contains("pro w9") { series = "9000"; }
    else if name.contains("rx 8") || name.contains("pro w8") || name.contains("radeon 8") { series = "8000"; }
    else if name.contains("rx 7") || name.contains("pro w7") || name.contains("radeon 7") { series = "7000"; }
    else if name.contains("rx 6") || name.contains("pro w6") || name.contains("radeon 6") { series = "6000"; }
    else if name.contains("rx 5") || name.contains("pro w5") { series = "5000"; }
    else if name.contains("mi50") || name.contains("radeon vii") { series = "MI50"; }
    else if name.contains("vega 64") || name.contains("vega frontier") { series = "VEGA64"; }
    else if name.contains("vega 56") { series = "VEGA56"; }

    match series {
        "9000" => ("11.0.0", "NVIDIA GeForce RTX 5090", "0"),
        "8000" => ("11.0.0", "NVIDIA GeForce RTX 4090", "1"),
        "7000" => ("11.0.0", "NVIDIA GeForce RTX 4090", "1"),
        "6000" => ("10.3.0", "NVIDIA GeForce RTX 3090 Ti", "1"),
        "5000" => ("10.1.0", "NVIDIA GeForce RTX 2080 Ti", "1"),
        "VEGA64" => ("9.0.0", "NVIDIA Tesla P100", "1"),
        "VEGA56" => ("9.0.0", "NVIDIA Tesla P100", "1"),
        "MI50" => ("9.0.6", "NVIDIA Tesla V100", "1"),
        _ => ("11.0.0", "NVIDIA GeForce RTX 4090", "1"),
    }
}

fn generate_nvml_stub(paths: &GhostPaths, spoof_name: &str, vram_bytes: u64, gpu_count: usize) {
    // FIX: Replaced the unconditional "import torch" with a lightweight import hook.
    // This prevents the infinite fork-bomb loop that was crashing the PC.
    let py_code = r#"
import builtins
import sys

_orig_import = builtins.__import__

def _ghost_import(name, globals=None, locals=None, fromlist=(), level=0):
    mod = _orig_import(name, globals, locals, fromlist, level)
    if name.startswith('torch'):
        if 'torch' in sys.modules:
            t = sys.modules['torch']
            if not getattr(t, '_ghost_patched', False):
                try:
                    t._ghost_patched = True
                    t.version.cuda = "12.4"
                    t.version.hip = "6.0"
                    if hasattr(t.backends, 'cuda'):
                        t.backends.cuda.is_built = lambda: True
                except Exception:
                    pass
    return mod

builtins.__import__ = _ghost_import
"#;
    let _ = fs::write(paths.spoof.join("sitecustomize.py"), py_code);
    let pythonpath = env::var("PYTHONPATH").unwrap_or_default();
    if !pythonpath.contains(paths.spoof.to_str().unwrap()) {
        unsafe { env::set_var("PYTHONPATH", format!("{};{}", paths.spoof.display(), pythonpath)); }
    }

    let meta_path = paths.spoof.join("stub.meta");
    let current_meta = format!("{}|{}|{}", spoof_name, vram_bytes, gpu_count);
    let nvml_path = paths.spoof.join("nvml.dll");
    let nvcuda_path = paths.spoof.join("nvcuda.dll");

    let mut needs_compile = true;
    if nvml_path.exists() && meta_path.exists() {
        if let Ok(saved_meta) = fs::read_to_string(&meta_path) {
            if saved_meta.trim() == current_meta { needs_compile = false; }
        }
    }

    if needs_compile && find_cl_exe() {
        println!("[GHOST] Hardware change detected. JIT Compiling Advanced NVML Hypervisor Stub...");
        
        let cpp_code = format!(r#"
#include <windows.h>
#include <string.h>
extern "C" {{
    __declspec(dllexport) int nvmlInit_v2() {{ return 0; }}
    __declspec(dllexport) int nvmlInit() {{ return 0; }}
    __declspec(dllexport) int nvmlShutdown() {{ return 0; }}
    __declspec(dllexport) int nvmlDeviceGetCount_v2(unsigned int *count) {{ *count = {}; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetHandleByIndex_v2(unsigned int index, void **device) {{ if(index < {}) {{ *device = (void*)(index + 1); return 0; }} return 1; }}
    __declspec(dllexport) int nvmlDeviceGetName(void *device, char *name, unsigned int length) {{ strncpy_s(name, length, "{}", _TRUNCATE); return 0; }}
    __declspec(dllexport) int nvmlDeviceGetMemoryInfo(void *device, unsigned long long *free, unsigned long long *total, unsigned long long *used) {{ 
        *total = {}ULL; *free = {}ULL; *used = 2147483648ULL; return 0; 
    }}
    __declspec(dllexport) int nvmlSystemGetDriverVersion(char *version, unsigned int length) {{ strncpy_s(version, length, "550.54", _TRUNCATE); return 0; }}
    __declspec(dllexport) int nvmlSystemGetNVMLVersion(char *version, unsigned int length) {{ strncpy_s(version, length, "12.550.54", _TRUNCATE); return 0; }}
    __declspec(dllexport) int nvmlDeviceGetTemperature(void *device, int sensorType, unsigned int *temp) {{ *temp = 45; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetPowerUsage(void *device, unsigned int *power) {{ *power = 150000; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetUtilizationRates(void *device, void *utilization) {{ if(utilization) {{ ((unsigned int*)utilization)[0] = 50; ((unsigned int*)utilization)[1] = 50; }} return 0; }}
    __declspec(dllexport) int nvmlDeviceGetUUID(void *device, char *uuid, unsigned int length) {{ strncpy_s(uuid, length, "GPU-12345678-1234-1234-1234-1234567890ab", _TRUNCATE); return 0; }}
    __declspec(dllexport) int nvmlDeviceGetFanSpeed(void *device, unsigned int *speed) {{ *speed = 30; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetPerformanceState(void *device, int *pState) {{ *pState = 0; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetArchitecture(void *device, int *arch) {{ *arch = 8; return 0; }}
    __declspec(dllexport) int nvmlDeviceGetCudaComputeCapability(void *device, int *major, int *minor) {{ *major = 8; *minor = 9; return 0; }}
    
    __declspec(dllexport) int cuInit(unsigned int Flags) {{ return 0; }}
    __declspec(dllexport) int cuDriverGetVersion(int *driverVersion) {{ *driverVersion = 12040; return 0; }}
    __declspec(dllexport) int cuDeviceGetCount(int *count) {{ *count = {}; return 0; }}
    __declspec(dllexport) int cuGetErrorString(int error, const char **pStr) {{ *pStr = "CUDA_SUCCESS"; return 0; }}
    __declspec(dllexport) int cuGetErrorName(int error, const char **pStr) {{ *pStr = "CUDA_SUCCESS"; return 0; }}
    __declspec(dllexport) int cuDeviceGetName(char *name, int len, int dev) {{ strncpy_s(name, len, "{}", _TRUNCATE); return 0; }}
    __declspec(dllexport) int cuDeviceTotalMem_v2(unsigned long long *bytes, int dev) {{ *bytes = {}ULL; return 0; }}
    __declspec(dllexport) int cuDeviceGetUuid(char *uuid, int dev) {{ strncpy_s(uuid, 16, "1234567890abcdef", _TRUNCATE); return 0; }}
    __declspec(dllexport) int cuDeviceComputeCapability(int *major, int *minor, int dev) {{ *major = 8; *minor = 9; return 0; }}
    __declspec(dllexport) int cuDeviceGetAttribute(int *pi, int attrib, int dev) {{ 
        if (attrib == 75) *pi = 8; else if (attrib == 76) *pi = 9; else if (attrib == 8) *pi = 128; else if (attrib == 1) *pi = 1024; else if (attrib == 2) *pi = 1024; else *pi = 1; return 0; 
    }}
}}
"#, gpu_count, gpu_count, spoof_name, vram_bytes, vram_bytes, gpu_count, spoof_name, vram_bytes);

        let temp_dir = env::temp_dir();
        let cpp_path = temp_dir.join("ghost_stub.cpp");
        let _ = fs::write(&cpp_path, cpp_code);

        let cl_status = Command::new("cl.exe")
            .args(&["/LD", "/O2", cpp_path.to_str().unwrap(), &format!("/Fe{}", nvml_path.display())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if cl_status.is_ok() && nvml_path.exists() {
            if symlink_file(&nvml_path, &nvcuda_path).is_err() {
                let _ = fs::copy(&nvml_path, &nvcuda_path);
            }
            let _ = fs::write(&meta_path, current_meta);
            println!("[GHOST] Advanced Hypervisor Stub Compiled and Cached.");
        }
    } else if !nvml_path.exists() {
        let version_dll = Path::new("C:\\Windows\\System32\\version.dll");
        let _ = fs::copy(version_dll, &nvml_path);
        let _ = fs::copy(version_dll, &nvcuda_path);
    }

    let current_path = env::var("PATH").unwrap_or_default();
    if !current_path.contains(paths.spoof.to_str().unwrap()) {
        unsafe { env::set_var("PATH", format!("{};{}", paths.spoof.display(), current_path)); }
    }
}

fn setup_environment(paths: &GhostPaths) -> (String, String) {
    let gpus = get_amd_gpus();
    let primary_name = if !gpus.is_empty() { gpus[0].name.clone() } else { "UNKNOWN".to_string() };
    let (gfx_mask, spoof_base, sdma) = get_mapping(&primary_name);
    
    let mut spoof_string = format!("{} (AMD Spoofed)", spoof_base);
    let mut hip_array = Vec::new();
    let mut total_vram = 0;

    for g in &gpus { 
        hip_array.push(g.os_index.to_string()); 
        total_vram += g.vram_bytes;
    }
    if total_vram == 0 { total_vram = 25769803776; }
    
    let mut hip_devices = "0".to_string();
    if gpus.len() > 1 {
        hip_devices = hip_array.join(",");
        spoof_string = format!("{} (Dual-GPU)", spoof_base);
    } else if gpus.len() == 1 {
        hip_devices = hip_array[0].clone();
    }

    unsafe {
        env::set_var("HSA_OVERRIDE_GFX_VERSION", gfx_mask);
        env::set_var("HIP_VISIBLE_DEVICES", &hip_devices);
        env::set_var("ROCR_VISIBLE_DEVICES", &hip_devices);
        env::set_var("CUDA_VERSION", "12.4");
        env::set_var("NVIDIA_VISIBLE_DEVICES", "all");
        env::set_var("GHOST_SPOOF_NAME", &spoof_string);
        env::set_var("HSA_ENABLE_SDMA", sdma);
    }

    generate_nvml_stub(paths, &spoof_string, total_vram, gpus.len().max(1));

    let current_path = env::var("PATH").unwrap_or_default();
    let mut new_paths = Vec::new();

    if let Some(zluda_exe) = find_file_recursive(&paths.zluda, "zluda.exe") {
        new_paths.push(zluda_exe.parent().unwrap().to_path_buf());
    }
    
    if let Some(perl_exe) = find_file_recursive(&paths.perl, "perl.exe") {
        new_paths.push(perl_exe.parent().unwrap().to_path_buf());
    }

    let rocm_base = Path::new("C:\\Program Files\\AMD\\ROCm");
    if rocm_base.exists() {
        if let Ok(entries) = fs::read_dir(rocm_base) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.exists() {
                    new_paths.push(bin_path);
                }
            }
        }
    }

    let mut path_string = current_path.clone();
    for p in new_paths {
        let p_str = p.to_str().unwrap();
        if !path_string.contains(p_str) {
            path_string = format!("{};{}", p_str, path_string);
        }
    }
    if path_string != current_path {
        unsafe { env::set_var("PATH", path_string); }
    }

    println!("[GHOST] Environment Active. {} GPU(s) Spoofed as {}. GFX Masked to {}.", gpus.len().max(1), spoof_string, gfx_mask);
    
    let vram_gb = format!("{:.1} GB", total_vram as f64 / 1073741824.0);
    (spoof_string, vram_gb)
}

// --- MEDIA CONTROLS ---
fn play_music(paths: &GhostPaths) {
    let track = paths.music.join("Ghost_Track_4.mp3");
    let vbs_path = paths.music.join("play.vbs");
    let vbs_code = format!(
        "Set Wmp = CreateObject(\"WMPlayer.OCX\")\nWmp.URL = \"{}\"\nWmp.settings.setMode \"loop\", True\nWmp.controls.play\nDo While Wmp.playState <> 1\nWScript.Sleep 1000\nLoop\nWScript.Sleep 3600000",
        track.display()
    );
    let _ = fs::write(&vbs_path, vbs_code);
    Command::new("wscript").arg(&vbs_path).spawn().ok();
    MUSIC_PLAYING.store(true, Ordering::SeqCst);
}

fn stop_music() {
    Command::new("taskkill").args(&["/F", "/IM", "wscript.exe"]).stdout(Stdio::null()).stderr(Stdio::null()).status().ok();
    MUSIC_PLAYING.store(false, Ordering::SeqCst);
}

fn toggle_music(paths: &GhostPaths) {
    if MUSIC_PLAYING.load(Ordering::SeqCst) {
        stop_music();
    } else {
        play_music(paths);
    }
}

fn launch_doom(paths: &GhostPaths) {
    if let Some(doom_exe) = find_file_recursive(&paths.doom, "chocolate-doom.exe") {
        let wad_file = paths.doom.join("DOOM1.WAD");
        if wad_file.exists() {
            Command::new(&doom_exe).arg("-iwad").arg(&wad_file).current_dir(doom_exe.parent().unwrap()).spawn().ok();
        }
    }
}

fn check_ready(pid: u32, log_path: &Path, tick: u32) -> bool {
    if let Ok(mut file) = File::open(log_path) {
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let start = if len > 8192 { len - 8192 } else { 0 };
        let _ = file.seek(SeekFrom::Start(start));
        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            let content = String::from_utf8_lossy(&buffer).to_lowercase();
            if content.contains("running on local url") || content.contains("uvicorn running on") || 
               content.contains("starting web server") || content.contains("http://127.0.0.1") || 
               content.contains("http://0.0.0.0") || content.contains("http://localhost") {
                return true;
            }
        }
    }
    
    if tick % 4 == 0 {
        if let Ok(output) = Command::new("netstat").args(&["-ano"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let pid_str = format!(" {}", pid);
            for line in stdout.lines() {
                if line.contains("LISTENING") && line.ends_with(&pid_str) {
                    return true;
                }
            }
        }
    }
    false
}

fn stream_log(log_path: &Path, child: &mut std::process::Child) {
    if let Ok(mut file) = File::open(log_path) {
        let mut buffer = [0u8; 4096];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => {
                    if let Ok(Some(_)) = child.try_wait() {
                        while let Ok(n) = file.read(&mut buffer) {
                            if n == 0 { break; }
                            print!("{}", String::from_utf8_lossy(&buffer[..n]));
                        }
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Ok(n) => {
                    print!("{}", String::from_utf8_lossy(&buffer[..n]));
                    io::stdout().flush().unwrap();
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }
}

fn waiting_room(paths: &GhostPaths, backend: &str, child: &mut std::process::Child, log_path: &Path, spoof_name: &str, total_vram_str: &str) -> bool {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide);
    terminal::enable_raw_mode().unwrap();

    let start_time = Instant::now();
    let mut crashed = false;
    let pid = child.id();

    let hw_stats = Arc::new(Mutex::new((String::from("N/A"), total_vram_str.to_string())));
    let hw_clone = Arc::clone(&hw_stats);
    let poll_running = Arc::new(AtomicBool::new(true));
    let poll_running_clone = Arc::clone(&poll_running);
    
    if Command::new("rocm-smi").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
        thread::spawn(move || {
            let re_temp = Regex::new(r"(\d+\.\d+)c|(\d+)c").unwrap();
            while poll_running_clone.load(Ordering::SeqCst) {
                if let Ok(out) = Command::new("rocm-smi").args(&["--showtemp"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    if let Some(caps) = re_temp.captures(&s) {
                        let temp = caps.get(1).or_else(|| caps.get(2)).map_or("N/A", |m| m.as_str());
                        let mut stats = hw_clone.lock().unwrap();
                        stats.0 = temp.to_string();
                    }
                }
                thread::sleep(Duration::from_secs(2));
            }
        });
    }

    let mut tick = 0;
    loop {
        if start_time.elapsed() > Duration::from_secs(600) {
            let _ = child.kill();
            crashed = true;
            break;
        }

        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() { crashed = true; }
            break;
        }

        if check_ready(pid, log_path, tick) {
            break;
        }

        let (temp, vram) = {
            let stats = hw_stats.lock().unwrap();
            (stats.0.clone(), stats.1.clone())
        };

        let _ = execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0));
        let _ = execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            Print("+==========================================================+\r\n"),
            Print("|                 "),
            SetForegroundColor(Color::Green),
            Print("GHOST ENVIRONMENT (RUST)"),
            SetForegroundColor(Color::Cyan),
            Print("                 |\r\n"),
            Print("+==========================================================+\r\n"),
            Print("|  STATUS: BOOTING AI MODEL...                             |\r\n"),
            Print(&format!("|  [ GPU: {:<46} ]|\r\n", spoof_name)),
            Print(&format!("|  [ TEMP: {:<5}C ]             [ VRAM: {:<16} ]|\r\n", temp, vram)),
            Print(&format!("|  [ BACKEND: {:<44} ]|\r\n", backend)),
            Print("+==========================================================+\r\n"),
            Print("|  HOTKEYS: [D] Play DOOM   [M] Toggle Music               |\r\n"),
            Print("+==========================================================+\r\n"),
            ResetColor
        );

        if event::poll(Duration::from_millis(250)).unwrap() {
            if let Event::Key(key_event) = event::read().unwrap() {
                match key_event.code {
                    KeyCode::Char('d') | KeyCode::Char('D') => launch_doom(paths),
                    KeyCode::Char('m') | KeyCode::Char('M') => toggle_music(paths),
                    _ => {}
                }
            }
        }
        tick += 1;
    }

    poll_running.store(false, Ordering::SeqCst);
    terminal::disable_raw_mode().unwrap();
    let _ = execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen);
    
    stop_music();
    MUSIC_PLAYING.store(false, Ordering::SeqCst);
    Command::new("taskkill").args(&["/F", "/IM", "chocolate-doom.exe"]).stdout(Stdio::null()).stderr(Stdio::null()).status().ok();
    
    crashed
}

fn translate_cuda(source_dir: &str, paths: &GhostPaths, aggressive: bool) {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("\n+==========================================================+");
    println!("|           GHOST HIPIFY SOURCE TRANSLATOR                 |");
    println!("+==========================================================+");
    let _ = execute!(stdout, ResetColor);

    let hipify_script = paths.hipify.join("hipify-perl");
    let src_path = Path::new(source_dir);
    if !src_path.exists() {
        println!("[GHOST] Error: Source folder '{}' does not exist.", source_dir);
        return;
    }

    let out_dir = src_path.parent().unwrap().join(format!("{}_hip_out", src_path.file_name().unwrap().to_string_lossy()));
    let _ = fs::create_dir_all(&out_dir);

    println!("[GHOST] Scanning '{}' for CUDA/C++ source files...", source_dir);
    
    let mut files_processed = 0;
    let mut warnings = 0;
    let mut has_inline_ptx = false;
    let mut has_warp_intrinsics = false;
    let mut has_texture_mem = false;
    let mut needs_aggressive = false;

    let mut dirs_to_visit = vec![src_path.to_path_buf()];
    let mut current_depth = 0;

    while !dirs_to_visit.is_empty() && current_depth < 4 {
        let mut next_dirs = Vec::new();
        for dir in dirs_to_visit {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        next_dirs.push(path);
                    } else if let Some(ext) = path.extension() {
                        if ext == "cu" || ext == "cpp" || ext == "cuh" || ext == "h" {
                            let out_file = out_dir.join(path.file_name().unwrap());
                            
                            if aggressive {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    let stripped = content.replace("#include <cooperative_groups.h>", "// stripped cooperative_groups")
                                                          .replace("cooperative_groups::", "/*cg*/");
                                    let _ = fs::write(&path, stripped);
                                }
                            }
                            
                            if let Ok(output) = Command::new("perl").arg(&hipify_script).arg(&path).output() {
                                let mut translated_code = String::from_utf8_lossy(&output.stdout).to_string();
                                translated_code = translated_code.replace("warpSize", "__AMDGCN_WAVEFRONT_SIZE");
                                
                                let stderr_str = String::from_utf8_lossy(&output.stderr);
                                
                                let _ = fs::write(&out_file, translated_code.as_bytes());
                                files_processed += 1;

                                if stderr_str.contains("warning") || stderr_str.contains("error") { warnings += 1; }
                                if stderr_str.contains("cooperative_groups") { needs_aggressive = true; }
                                if translated_code.contains("asm(") || translated_code.contains("asm volatile") { has_inline_ptx = true; }
                                if translated_code.contains("__shfl") { has_warp_intrinsics = true; }
                                if translated_code.contains("texture<") { has_texture_mem = true; }
                            }
                        }
                    }
                }
            }
        }
        dirs_to_visit = next_dirs;
        current_depth += 1;
    }

    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("\n--- TRANSLATION REPORT ---");
    let _ = execute!(stdout, ResetColor);
    println!("Files Processed: {}", files_processed);
    
    let mut converted_pct = 100.0;
    if files_processed > 0 {
        let penalty = (warnings as f32 / (files_processed as f32 * 5.0)) * 100.0;
        converted_pct = (100.0 - penalty).clamp(0.0, 100.0);
    }
    println!("Converted:       {:.1}%", converted_pct);
    
    let _ = execute!(stdout, SetForegroundColor(Color::Yellow));
    println!("Warnings:        {}", warnings);
    let _ = execute!(stdout, ResetColor);

    if needs_aggressive && !aggressive {
        let _ = execute!(stdout, SetForegroundColor(Color::Red));
        println!("\n[!] CUDA 12.6 Features Detected (cooperative_groups).");
        println!("[!] HIPIFY failed to parse some files. Run 'translate <folder> --aggressive' to strip them.");
        let _ = execute!(stdout, ResetColor);
    }

    if has_inline_ptx || has_warp_intrinsics || has_texture_mem {
        let _ = execute!(stdout, SetForegroundColor(Color::Red));
        println!("Unsupported Features Detected:");
        if has_inline_ptx { println!("  - Inline PTX (Fix: Rewrite assembly using native HIP/C++ functions)"); }
        if has_warp_intrinsics { println!("  - Warp Intrinsics (Fix: AMD uses Wave64. Update warp size assumptions from 32 to __AMDGCN_WAVEFRONT_SIZE)"); }
        if has_texture_mem { println!("  - Texture Memory (Fix: Replace legacy CUDA texture references with HIP texture objects)"); }
        let _ = execute!(stdout, ResetColor);
    } else {
        println!("Unsupported:     None Detected");
    }
    
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("--------------------------");
    let _ = execute!(stdout, ResetColor);
}

fn run_doctor(paths: &GhostPaths) {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("\n+==========================================================+");
    println!("|                 GHOST DOCTOR DIAGNOSTICS                 |");
    println!("+==========================================================+");
    let _ = execute!(stdout, ResetColor);

    let rocm_exists = Path::new("C:\\Program Files\\AMD\\ROCm").exists();

    let checks = vec![
        ("MSVC Compiler (cl.exe)", Command::new("cl.exe").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() || find_cl_exe()),
        ("AMD Compiler (hipcc)", Command::new("hipcc").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() || rocm_exists),
        ("ROCm SMI (rocm-smi)", Command::new("rocm-smi").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() || rocm_exists),
        ("Strawberry Perl", Command::new("perl").arg("-v").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() || find_file_recursive(&paths.perl, "perl.exe").is_some()),
        ("PyTorch (import torch)", Command::new("python").args(&["-c", "import torch"]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()),
    ];

    for (name, passed) in checks {
        if passed {
            let _ = execute!(stdout, SetForegroundColor(Color::Green));
            println!("  [√] {}", name);
        } else {
            let _ = execute!(stdout, SetForegroundColor(Color::Red));
            println!("  [X] {}", name);
            
            let _ = execute!(stdout, SetForegroundColor(Color::Yellow));
            match name {
                "MSVC Compiler (cl.exe)" => println!("      -> Fix: Run 'install-deps' to download Visual Studio Build Tools"),
                "AMD Compiler (hipcc)" => println!("      -> Fix: Install AMD HIP SDK"),
                "ROCm SMI (rocm-smi)" => println!("      -> Fix: Install AMD HIP SDK"),
                "Strawberry Perl" => println!("      -> Fix: Run 'install-deps' to download Strawberry Perl"),
                "PyTorch (import torch)" => println!("      -> Fix: Run 'pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/rocm6.0'"),
                _ => {}
            }
        }
    }
    let _ = execute!(stdout, ResetColor);
    println!();
}

fn run_benchmark() {
    let py_code = r#"
import torch
import time

print('[GHOST] Warming up GPU and boosting clocks...')
try:
    # Use 8192x8192 (powers of 2 are highly optimized on GPUs)
    size = 8192
    a = torch.randn(size, size, dtype=torch.float32, device='cuda')
    b = torch.randn(size, size, dtype=torch.float32, device='cuda')
    
    # Heavy warmup to force GPU out of idle state
    for _ in range(10):
        _ = torch.matmul(a, b)
    torch.cuda.synchronize()

    print(f'[GHOST] Running FP32 (Standard) GEMM Benchmark ({size}x{size})...')
    start = time.time()
    iters = 20
    for _ in range(iters):
        c = torch.matmul(a, b)
    torch.cuda.synchronize()
    end = time.time()
    
    # TFLOPS formula: (2 * M * N * K * iterations) / (time * 10^12)
    tflops32 = (iters * 2.0 * size**3) / ((end - start) * 1e12)
    print(f'[GHOST] FP32 RESULT: {tflops32:.2f} TFLOPS')

    print(f'\n[GHOST] Running FP16 (Matrix Core) GEMM Benchmark ({size}x{size})...')
    a16 = a.half()
    b16 = b.half()
    torch.cuda.synchronize()
    
    start = time.time()
    for _ in range(iters):
        c16 = torch.matmul(a16, b16)
    torch.cuda.synchronize()
    end = time.time()
    
    tflops16 = (iters * 2.0 * size**3) / ((end - start) * 1e12)
    print(f'[GHOST] FP16 RESULT: {tflops16:.2f} TFLOPS')

except Exception as e:
    print(f'[GHOST] Benchmark failed: {e}')
"#;
    let temp_dir = env::temp_dir();
    let py_path = temp_dir.join("ghost_bench.py");
    let _ = fs::write(&py_path, py_code);

    Command::new("python").arg(&py_path).status().ok();
    let _ = fs::remove_file(py_path);
}

fn run_ai(script: &str, args: &[String], paths: &GhostPaths, spoof_name: &str, vram_str: &str, use_tui: bool) {
    println!("[GHOST] Attempting Native ROCm Execution...");
    
    let log_path = env::temp_dir().join(format!("ghost_ai_{}.log", std::process::id()));
    let crashed;

    if use_tui {
        let log_file = File::create(&log_path).unwrap();
        let err_file = log_file.try_clone().unwrap();

        let mut child = Command::new("python")
            .arg(script)
            .args(args)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .spawn()
            .expect("Failed to start Python process");

        crashed = waiting_room(paths, "ROCm Native", &mut child, &log_path, spoof_name, vram_str);
        
        if !crashed {
            stream_log(&log_path, &mut child);
        }
    } else {
        let mut child = Command::new("python")
            .arg(script)
            .args(args)
            .spawn()
            .expect("Failed to start Python process");
        
        let status = child.wait().expect("Failed to wait on child");
        crashed = !status.success();
    }
    
    if crashed {
        println!("\n[GHOST] Crash detected. Initiating ZLUDA Failover...");
        
        let zluda_exe = find_file_recursive(&paths.zluda, "zluda.exe");
        let zluda_dll = find_file_recursive(&paths.zluda, "nvcuda.dll");
        
        if zluda_exe.is_none() && zluda_dll.is_none() {
            println!("[GHOST] CRITICAL: ZLUDA executable/DLL not found. Cannot failover.");
            return;
        }

        if use_tui {
            let log_file_z = File::create(&log_path).unwrap();
            let err_file_z = log_file_z.try_clone().unwrap();

            let mut zluda_child = if let Some(exe_path) = zluda_exe {
                let mut zluda_args = vec!["--".to_string(), "python".to_string(), script.to_string()];
                zluda_args.extend_from_slice(args);
                Command::new(&exe_path)
                    .args(&zluda_args)
                    .stdout(Stdio::from(log_file_z))
                    .stderr(Stdio::from(err_file_z))
                    .spawn()
                    .expect("Failed to start ZLUDA")
            } else {
                let dll_path = zluda_dll.unwrap();
                let current_path = env::var("PATH").unwrap_or_default();
                let new_path = format!("{};{}", dll_path.parent().unwrap().display(), current_path);
                unsafe { env::set_var("PATH", new_path); }
                Command::new("python")
                    .arg(script)
                    .args(args)
                    .stdout(Stdio::from(log_file_z))
                    .stderr(Stdio::from(err_file_z))
                    .spawn()
                    .expect("Failed to start Python with ZLUDA DLLs")
            };

            let z_crashed = waiting_room(paths, "ZLUDA Translation", &mut zluda_child, &log_path, spoof_name, vram_str);
            if !z_crashed {
                stream_log(&log_path, &mut zluda_child);
            }
        } else {
            let mut zluda_child = if let Some(exe_path) = zluda_exe {
                let mut zluda_args = vec!["--".to_string(), "python".to_string(), script.to_string()];
                zluda_args.extend_from_slice(args);
                Command::new(&exe_path)
                    .args(&zluda_args)
                    .spawn()
                    .expect("Failed to start ZLUDA")
            } else {
                let dll_path = zluda_dll.unwrap();
                let current_path = env::var("PATH").unwrap_or_default();
                let new_path = format!("{};{}", dll_path.parent().unwrap().display(), current_path);
                unsafe { env::set_var("PATH", new_path); }
                Command::new("python")
                    .arg(script)
                    .args(args)
                    .spawn()
                    .expect("Failed to start Python with ZLUDA DLLs")
            };
            let _ = zluda_child.wait();
        }
    }
    
    let _ = fs::remove_file(&log_path);
}

fn clean_registry() {
    let path = "SOFTWARE\\NVIDIA Corporation";
    let _ = RegKey::predef(HKEY_LOCAL_MACHINE).delete_subkey_all(path);
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(path);
}

fn clean_folder(paths: &GhostPaths) {
    if paths._base.exists() {
        if let Err(e) = fs::remove_dir_all(&paths._base) {
            println!("[!] Could not fully delete .ghost folder: {}", e);
            println!("[!] Make sure no other programs (like Python or ZLUDA) are using it.");
        }
    }
}

fn run_clean(paths: &GhostPaths) {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, SetForegroundColor(Color::Cyan));
    println!("\n+==========================================================+");
    println!("|                 GHOST SYSTEM CLEANER                     |");
    println!("+==========================================================+");
    let _ = execute!(stdout, ResetColor);
    println!("1. Remove Registry Spoofs (NVIDIA Corporation keys)");
    println!("2. Delete .ghost folder (Requires re-downloading dependencies)");
    println!("3. Nuke Everything (Registry + .ghost folder)");
    println!("4. Cancel");
    print!("\nSelect an option (1-4): ");
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).unwrap();
    
    match choice.trim() {
        "1" => {
            clean_registry();
            let _ = execute!(stdout, SetForegroundColor(Color::Green));
            println!("[√] Registry spoofs removed.");
            let _ = execute!(stdout, ResetColor);
        }
        "2" => {
            clean_folder(paths);
            let _ = execute!(stdout, SetForegroundColor(Color::Green));
            println!("[√] .ghost folder removed.");
            let _ = execute!(stdout, ResetColor);
        }
        "3" => {
            clean_registry();
            clean_folder(paths);
            let _ = execute!(stdout, SetForegroundColor(Color::Green));
            println!("[√] All Ghost traces removed.");
            let _ = execute!(stdout, ResetColor);
        }
        _ => {
            println!("Cancelled.");
        }
    }
}

fn run_shell(paths: &GhostPaths, spoof_name: &str, vram_str: &str) {
    let mut stdout = io::stdout();
    
    loop {
        let current_dir = env::current_dir().unwrap();
        let _ = execute!(stdout, SetForegroundColor(Color::Green), Print("(ghost-amd) "), ResetColor);
        print!("{}> ", current_dir.display());
        stdout.flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() { continue; }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0];

        match command {
            "exit" | "quit" => break,
            "cd" => {
                if parts.len() > 1 {
                    let new_dir = parts[1..].join(" ");
                    if let Err(e) = env::set_current_dir(&new_dir) {
                        println!("cd: {}: {}", new_dir, e);
                    }
                }
            }
            "ls" | "dir" => {
                if let Ok(entries) = fs::read_dir(current_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = path.file_name().unwrap().to_string_lossy();
                        if path.is_dir() {
                            let _ = execute!(stdout, SetForegroundColor(Color::Blue), Print(format!("{}/ \n", name)), ResetColor);
                        } else if name.ends_with(".py") {
                            let _ = execute!(stdout, SetForegroundColor(Color::Yellow), Print(format!("{} \n", name)), ResetColor);
                        } else {
                            println!("{}", name);
                        }
                    }
                }
            }
            "run" => {
                if parts.len() > 1 {
                    let mut use_tui = true;
                    let mut script_idx = 1;
                    
                    if parts[1] == "--no-tui" {
                        use_tui = false;
                        script_idx = 2;
                    }
                    
                    if parts.len() > script_idx {
                        let script = parts[script_idx];
                        let args: Vec<String> = parts[script_idx + 1..].iter().map(|s| s.to_string()).collect();
                        run_ai(script, &args, paths, spoof_name, vram_str, use_tui);
                    } else {
                        println!("Usage: run [--no-tui] <script.py> [args]");
                    }
                } else {
                    println!("Usage: run [--no-tui] <script.py> [args]");
                }
            }
            "translate" => {
                if parts.len() > 1 {
                    let aggressive = parts.contains(&"--aggressive");
                    translate_cuda(parts[1], paths, aggressive);
                } else {
                    println!("Usage: translate <folder_path> [--aggressive]");
                }
            }
            "benchmark" => {
                run_benchmark();
            }
            "doctor" => {
                run_doctor(paths);
            }
            "install-deps" => {
                ensure_dependencies(paths);
            }
            "clean" => {
                run_clean(paths);
            }
            "ghost" | "ghost.exe" | "./ghost" | ".\\ghost.exe" => {
                let _ = execute!(stdout, SetForegroundColor(Color::Yellow));
                println!("You are already inside the Ghost shell! (Prevented nested execution to save RAM/CPU)");
                let _ = execute!(stdout, ResetColor);
            }
            _ => {
                let mut child = Command::new(command);
                child.args(&parts[1..]);
                if child.status().is_err() {
                    Command::new("cmd").args(&["/C", input]).status().ok();
                }
            }
        }
    }
}

fn main() {
    let _reg_guard = RegistryGuard::new();

    println!("Initializing Ghost Environment (Rust Edition)...");
    let paths = GhostPaths::new();
    paths.init();
    
    if find_file_recursive(&paths.zluda, "nvcuda.dll").is_none() && find_file_recursive(&paths.zluda, "zluda.exe").is_none() {
        ensure_dependencies(&paths);
    }
    
    let (spoof_name, vram_str) = setup_environment(&paths);
    
    println!("Environment Active. Dropping into Ghost Shell.");
    println!("Commands: 'ls', 'cd', 'run [--no-tui] <script.py>', 'translate <folder>', 'benchmark', 'doctor', 'install-deps', 'clean'");
    
    run_shell(&paths, &spoof_name, &vram_str);
}