# AMD Ghost Environment (v2.0)

## Overview
The AMD Ghost Environment is a powerful, environment-level daemon designed to bridge the gap between CUDA-centric AI software and AMD ROCm hardware. By injecting a targeted set of environment variables and dynamic translation libraries at runtime, this tool allows various AMD RDNA architectures to report their identity as high-end NVIDIA GeForce RTX equivalents.

Version 2.0 Massive Update: Ghost is no longer just an environment wrapper. It is now a fully automated Virtual Environment Daemon featuring Smart Execution Logic. If an AI application fails to run natively on ROCm, Ghost will instantly catch the crash, suppress it, and seamlessly inject the ZLUDA translation layer to run NVIDIA-only CUDA binaries directly on your AMD hardware.

ROADMAP:

Completed: Automated ZLUDA Integration, Smart Failover Logic, Interactive TUI, Auto-Dependency Installer.

Completed: Full Windows Native Support
Goal: Bring the entire Ghost Environment, including ZLUDA translation, hardware spoofing, and the Waiting Room TUI natively to Windows.
Status: Achieved via a hardened PowerShell daemon with WMI/DXDiag hardware polling and native Windows API integration.

Note: Development is currently balanced alongside full-time studies, so updates may be paced accordingly to ensure stability over speed.

## System Requirements & Prerequisites

**Windows:**
* **OS:** Windows 10 or Windows 11.
* **Drivers:** Latest AMD Software: Adrenalin Edition or PRO Edition.
* **AMD HIP SDK:** *Crucial* for native ROCm execution and allows Ghost to accurately poll VRAM and Temperature data.
* **PowerShell:** Version 5.1 or newer (Built into Windows).

**Linux:**
* **OS:** Ubuntu 22.04/24.04 or compatible Debian-based distro.
* **Drivers:** AMD ROCm dkms drivers.
* **Audio:** `mpg123` (Required for the Waiting Room music).

## Hardware Support Matrix
The internal lookup table (mapping.json) provides dynamic translation logic across multiple generations of AMD and NVIDIA architectures:

| AMD Host Series | Mask Version | NVIDIA Spoof Target |
|:--- |:--- |:--- |
| **RX 9000 Series** | 11.0.0 | NVIDIA GeForce RTX 5090 |
| **RX 7000 Series** | 11.0.0 | NVIDIA GeForce RTX 4090 |
| **RX 6000 Series** | 10.3.0 | NVIDIA GeForce RTX 3090 Ti |
| **RX 5000 Series** | 10.1.0 | NVIDIA GeForce RTX 2080 Ti |

*(Note: GFX Version on RDNA 4 cards has been masked from 12.0 to 11.0. This is because the architecture is too new, and many AI libraries currently lack native support. Masking to 11.0 ensures maximum compatibility.)*

## Key Features
* **Smart Execution Logic (ROCm -> ZLUDA):** Ghost attempts to run your AI natively in ROCm first. If it detects a crash (e.g., "No HIP backend" or "CUDA out of memory"), it automatically intercepts the crash and restarts the app using ZLUDA CUDA-to-HIP translation.
* **The Waiting Room TUI:** While your heavy AI models load in the background, Ghost provides an interactive terminal UI featuring live Hardware Polling (Temp/VRAM).
* **Integrated DOOM & Music:** Bored while waiting for a 10GB model to load into VRAM? Press D to play a fully playable, perfectly scaled version of doom-ascii, or press M to stream royalty-free background music.
* **Auto-Repair Engine:** If your ZLUDA binaries, DOOM dependencies, or audio players are missing or corrupted, Ghost will automatically download, compile, and repair them on the fly.
* **First-Startup Wizard:** Automatically applies specific performance patches based on your UI of choice (SwarmUI, SD.Next, Forge/A1111, vLLM).

## Installation

### Linux
1. Clone the repository to your local machine:
```bash
git clone https://github.com/Void-Compute/AMD-Ghost-Enviroment.git
cd AMD-Ghost-Enviroment
```

2. Ensure the wrapper script is executable:
```bash
chmod +x bin/ghost
```

3. Install the Ghost Daemon globally (Requires sudo):
```bash
sudo ./bin/ghost install
```
*(This creates a secure symbolic link, allowing you to type `ghost` from anywhere on your system).*

### Windows
1. Clone or download the repository to your local machine.
2. Open the folder containing the Windows script.
3. Right-click `ghost.ps1` and select **Run with PowerShell**.

## Usage

### 1. Enter the Ghost Environment

## Quick Start
To initialize the system:
1. **Open your File Explorer.**
2. **Navigate to the Ghost folder.**
3. **Double-click the 'ghost' script file (Linux) or run 'ghost.ps1' (Windows).** (Choose 'Run in Terminal' if prompted).

The environment will automatically resolve the correct directory, activate your Python venv, and apply all AMD/NVIDIA spoofing masks.

*On your first run, the First-Startup Wizard will ask you which AI tool you are using and automatically download the ZLUDA translation engine.*

### 2. Launch Your AI Application
Once inside the `(ghost-env)` terminal, navigate to your AI folder (e.g., SwarmUI or Forge). To launch your script with the Smart Failover and Waiting Room TUI, prefix your command with `ghost-start`:
```bash
ghost-start python3 launch.py
```

### 3. The Waiting Room
While your AI loads, you will see the Ghost TUI. 
* Press **M** to toggle background music.
* Press **N** to open the Music Downloader (Download Synthwave, Lofi, or EDM).
* Press **D** to launch DOOM. 
* *The exact millisecond your AI finishes loading and opens its web port, Ghost will auto-save DOOM, stop the music, and hand control back to your AI!*

## Verification Test

To confirm that the wrapper is successfully spoofing the hardware identity and linking to ROCm, run the following Python diagnostic command inside the ghost environment:

```bash
python3 -c "import torch, os; print('\n--- SYSTEM DIAGNOSTIC ---\nCUDA Available:', torch.cuda.is_available(), '\nHardware Device:', torch.cuda.get_device_name(0) if torch.cuda.is_available() else 'None', '\nSpoofed Identity:', os.getenv('__GL_RENDERER_STRING'), '\nROCm Version:', torch.version.hip, '\n-------------------------')"
```

## Troubleshooting & Debugging

**Windows: "Running scripts is disabled on this system"**
By default, Windows restricts custom PowerShell scripts. Open PowerShell as Administrator and run:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

**Windows: VRAM/Temp stuck on "Loading..."**
Ensure the **AMD HIP SDK** is installed and your display drivers are up to date. The Windows version of Ghost relies on `dxdiag` and WMI to poll hardware stats.

* **System Link**: Note that the `ghost` simple systemlink command (installing to /usr/local/bin) is currently a **Work In Progress (WIP)**. If the global command fails, please revert to the double-click method.

**DOOM looks like a jumbled mess of letters!**
Bash cannot physically change your GUI font size. If DOOM looks stretched or unreadable:
1. Change Terminal Size to 80x120
2. Press CTRL + - (Minus) 5 or 6 times to zoom out. Smaller fonts = Higher Resolution ASCII graphics!

**The Music won't play / No Audio**
Ghost uses mpg123 to bypass strict Linux audio servers. If music fails to play, ensure you have the dependencies installed:
```bash
sudo apt install mpg123
```

**ZLUDA Failover didn't trigger**
Ensure you are launching your script using `ghost-start python3 ...` and not just `python3`. The `ghost-start` command is required to wrap the process in the Watchdog and Failover logic.

**"ghost: command not found"**
If the global installation failed, you can run it via absolute path:
```bash
~/AMD-Ghost-Environment/bin/ghost
```

## Technical Architecture
The ghost daemon utilizes the following primary variables and techniques to achieve hardware parity:
* **HSA_OVERRIDE_GFX_VERSION**: Defines the target AMD architecture for ROCm compatibility.
* **HSA_ENABLE_SDMA=0**: Disables PCIe atomics to prevent crashes on newer RDNA architectures.
* **LD_PRELOAD Injection (Linux) / PATH Injection (Windows)**: Dynamically injects ZLUDA into the Python process tree, intercepting CUDA calls and translating them to AMD HIP in real-time.
* **PTY Isolation**: Uses Pseudo-Terminals (Linux) and hidden process redirection (Windows) to isolate background processes, ensuring the TUI and DOOM have raw, uninterrupted access to keyboard inputs.

## Disclaimer
This project is for educational and developmental purposes. Hardware spoofing and binary translation may violate the terms of service of certain proprietary applications. Use responsibly.

---

## Ghost-Wrapper: Support the Grind

This tool exists because we shouldn't have to pay the "NVIDIA Tax" to run top-tier AI. If Ghost-Wrapper saved you from a 50-step Linux tutorial, consider supporting the project.

As a solo student dev, I keep things independent. No banks, no trackers - just code and caffeine.

### Digital Tokens (Direct Support)
| Asset | Address |
| :--- | :--- |
| **Ethereum (ETH)** | 0x9e046b3fa85932351b34520837d95bc1ad309748 |
| **Bitcoin (BTC)** | bc1q30wftehky6ct06pshrjh3ekhe9as9uzzdemd0r |

### Social Support
* **Star this Repo:** It is the best way to help the project grow.
* **Report a Bug:** Help me iron out the edge cases.

*Keeping the RDNA dream alive, one wrapper at a time.*