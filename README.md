```
# AMD Ghost Environment (v1.56  Rust Edition)

## Overview
The AMD Ghost Environment is a high performance environment level daemon designed to bridge the gap between CUDA centric AI software and AMD ROCm hardware. By injecting a targeted set of environment variables and dynamic translation libraries at runtime this tool allows various AMD RDNA architectures to report their identity as high end NVIDIA GeForce RTX equivalents.

**Version 1.56 Massive Update (The Rust Rewrite):** Ghost has been completely rewritten from PowerShell into a high performance memory safe Rust binary. It is no longer just a wrapper; it is a fully automated Virtual Environment Shell featuring:
* **Smart Execution Logic:** Auto detects crashes and injects ZLUDA failover.
* **JIT Compilation:** On the fly generation of NVML/CUDA hypervisor stubs.
* **Ghost Shell:** A dedicated sub shell environment for AI development.

**ROADMAP:**
* **Completed:** Automated ZLUDA Integration Smart Failover Logic Interactive TUI.
* **Completed:** Full Windows Native Support (Hardened Rust daemon with Registry/WMI hooks).
* **Completed:** Built in interactive (ghost amd) shell with doctor benchmark and translate.
* **WIP:** Linux version is currently in development and may lack parity with the Windows Rust build.
* Note: Development is balanced alongside full time studies; stability is prioritized over speed.

---

## CRITICAL: Installation & Usage

**YOU MUST RUN THIS PROGRAM AS ADMINISTRATOR.** The Ghost Engine requires elevated privileges to perform dynamic Windows Registry spoofing system level symlinking and hardware polling.

### 1. Launching
* **Right click ghost_amd.exe and select "Run as Administrator".**
* On first launch Ghost will automatically download required dependencies (ZLUDA HIP SDK stubs etc.).

### 2. The Ghost Shell Commands
Once inside the (ghost amd)> prompt use these built in tools:
* `run <script.py> [args]`: Launches AI scripts with Smart Failover and the Waiting Room TUI.
* `translate <folder>`: Uses the Perl HIPIFY engine to convert CUDA C++/Python code to native AMD HIP.
* `benchmark`: Tests your GPU's actual TFLOPS performance (FP16/FP32).
* `doctor`: Diagnoses your environment (checks for MSVC cl.exe ROCm and Drivers).
* `install-deps`: Manually triggers a fresh download of all core Ghost components.
* `clean`: Removes registry spoofs and nukes the .ghost environment folder.

---

## Hardware Support Matrix
Ghost uses dynamic translation logic to map your AMD hardware to the closest NVIDIA equivalent for library compatibility:

| AMD Host Series | Mask Version | NVIDIA Spoof Target |
|:--- |:--- |:--- |
| **RX 9000 Series** | 11.0.0 | NVIDIA GeForce RTX 5090 |
| **RX 8000 Series** | 11.0.0 | NVIDIA GeForce RTX 4090 |
| **RX 7000 Series** | 11.0.0 | NVIDIA GeForce RTX 4090 |
| **RX 6000 Series** | 10.3.0 | NVIDIA GeForce RTX 3090 Ti |
| **RX 5000 Series** | 10.1.0 | NVIDIA GeForce RTX 2080 Ti |
| **Radeon VII / MI50** | 9.0.6 | NVIDIA Tesla V100 |
| **Vega 64 / Vega 56** | 9.0.0 | NVIDIA Tesla P100 |

---

## Key Features

### The Waiting Room TUI
While your heavy AI models (like Stable Diffusion or LLMs) load Ghost provides an interactive dashboard:
* **Live Polling:** Real time VRAM Temperature and GPU Load stats.
* **Integrated DOOM:** Press **D** to play a fully functional version of DOOM inside the shell while waiting.
* **Background Music:** Press **M** to toggle a background stream.
* The exact millisecond your AI finishes loading and opens its local web port Ghost will hand control back to your AI!

### Smart Execution Logic (ROCm  ZLUDA)
Ghost tries to run your AI natively via AMD ROCm first for maximum performance. If a crash or incompatibility is detected it automatically restarts the process with the **ZLUDA translation layer** injected allowing NVIDIA only binaries to run on your RDNA card.

---

## Technical Architecture
* **JIT Stub Generation:** Writes and compiles C++ code using cl.exe in real time to generate an nvml.dll that hardcodes your specific AMD VRAM and GPU name into NVIDIA memory queries.
* **HSA_OVERRIDE_GFX_VERSION:** Forces the OS to recognize unsupported RDNA versions as compatible ROCm targets.
* **Python Import Hooks:** Injects sitecustomize.py to intercept import torch and force cuda.is_available() to return True.
* **Registry Guard:** Safely manages the SOFTWARE\NVIDIA Corporation keys to prevent system wide corruption while ensuring AI apps see a valid "NVIDIA" environment.

---

## Troubleshooting

**"MSVC Compiler (cl.exe) is missing"**
The JIT engine needs the C++ Build Tools. Run doctor and follow the link to install the Visual Studio Build Tools (C++).

**"Access Denied"**
Close the program and **Run as Administrator**.

**"Linux version not working"**
The Linux build is currently a Work In Progress (WIP). Please use the Windows Rust build for the most stable experience.

---

## Support the Grind
This project is developed by a solo 15 year old dev to fight the "NVIDIA Tax." No banks no trackers  just code and caffeine.

**Ethereum (ETH):** `0x9e046b3fa85932351b34520837d95bc1ad309748`
**Bitcoin (BTC):** `bc1q30wftehky6ct06pshrjh3ekhe9as9uzzdemd0r`

*Keeping the RDNA dream alive one iteration at a time.*
```