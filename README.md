# AMD Ghost Environment

## Overview
The AMD Ghost Environment is a lightweight, environment-level wrapper designed to facilitate compatibility between CUDA-centric software and AMD ROCm hardware. By injecting a targeted set of environment variables at runtime, this tool allows various AMD RDNA architectures to report their identity as high-end NVIDIA GeForce RTX equivalents.

This prevents hard-coded architecture checks from blocking execution and ensures that libraries like PyTorch and ONNX Runtime route their workloads through the AMD ROCm compute layer via the `HSA_OVERRIDE_GFX_VERSION` mask.

Note on Technical Scope: This is not a binary translator (like ZLUDA). Instead, it uses ISA Target Spoofing and environment-level overrides to allow ROCm-enabled applications to run on GFX IDs that are not officially in their support matrix. Still enough to run about 90% of the locked Software (ZLUDA may be added in the future)

ROADMAP:

Experimental: ZLUDA Integration 

Expected release
Q3 2026. 

Note: Development is currently balanced alongside full-time studies, so updates may be paced accordingly to ensure stability over speed.

Goal: Move beyond "Target Spoofing" and allow the Ghost Environment to run NVIDIA-only CUDA binaries on AMD hardware using the ZLUDA translation layer.

Status: Currently researching automated binary injection. This will allow the wrapper to detect if an application lacks a ROCm path and automatically pivot to CUDA translation on the fly.

## Hardware Support Matrix
The internal lookup table (`mapping.json`) provides translation logic across multiple generations of AMD and NVIDIA architectures:

| AMD Host Series | Mask Version | NVIDIA Spoof Target |
|:--- |:--- |:--- |
| **RX 9000 Series** | 11.0.0 | NVIDIA GeForce RTX 5090 |
| **RX 7000 Series** | 11.0.0 | NVIDIA GeForce RTX 4090 |
| **RX 6000 Series** | 10.3.0 | NVIDIA GeForce RTX 3090 Ti |
| **RX 5000 Series** | 10.1.0 | NVIDIA GeForce RTX 2080 Ti |

*(Note: GFX Version on RDNA 4 cards has been changed from 12.0 to 11.0 due to missing compatibility in most libraries because the card is too new and many libs currently dont have the necessary support)*

## Key Features
* **Multi-Generational Support:** Pre-configured mappings for RDNA 1 through RDNA 4.
* **Compute Hijacking:** Injects necessary ROCm architecture masks to execute CUDA instructions on AMD hardware.
* **Non-Destructive Integration:** Operates purely as a command-line wrapper without modifying global system states.
* **Virtual Environment Support:** Fully compatible with Python `venv`, Conda, and isolated containers.


## Installation

1. Clone the repository to your local machine:
```bash
git clone https://github.com/ChrisGamer5013/AMD-Ghost-Environment.git
cd AMD-Ghost-Environment
```

2. Ensure the wrapper script is executable:
```bash
chmod +x bin/ghost
```

3. (Optional) Create a symbolic link to make it accessible system-wide:
```bash
sudo ln -s ~/AMD-Ghost-Environment/bin/ghost /usr/local/bin/ghost
```

## Usage

To utilize the wrapper, prefix any standard terminal command with `ghost`. The target application will launch within the masked environment.

### 1. Running Python Scripts
```bash
ghost python3 main.py
```

### 2. Installing Dependencies
It is highly recommended to run package installations through the wrapper to ensure compile-time hardware checks pass successfully.
```bash
ghost pip install torch torchvision torchaudio --index-url [https://download.pytorch.org/whl/rocm7.2](https://download.pytorch.org/whl/rocm7.2)
```

### 3. Interactive Shell Mode
If you need to run multiple commands without prefixing each one, simply type `ghost` to enter a persistent masked shell session. Type `exit` when finished.
```bash
ghost
```

## Verification Test

To confirm that the wrapper is successfully spoofing the hardware identity and linking to ROCm, run the following Python diagnostic command inside a virtual environment:

```bash
ghost python3 -c "import torch, os; print('\n--- SYSTEM DIAGNOSTIC ---\nCUDA Available:', torch.cuda.is_available(), '\nHardware Device:', torch.cuda.get_device_name(0) if torch.cuda.is_available() else 'None', '\nSpoofed Identity:', os.getenv('__GL_RENDERER_STRING'), '\nROCm Version:', torch.version.hip, '\n-------------------------')"
```

**Expected Output:**
* `CUDA Available: True`
* `Spoofed Identity: NVIDIA GeForce RTX 4090`
* `ROCm Version: 7.2` (or your current ROCm version)

### Troubleshooting: "ghost: command not found"
If your terminal does not recognize the `ghost` command (common inside certain virtual environments or shells), use the absolute path to the wrapper instead:

```bash
~/AMD-Ghost-Environment/bin/ghost <your-command>
```
##Alternatively, you can temporarily alias it for your current session:
```bash
alias ghost='~/AMD-Ghost-Environment/bin/ghost'
```

## Technical Architecture
The `ghost` script utilizes the following primary variables to achieve hardware parity:
* `HSA_OVERRIDE_GFX_VERSION`: Defines the target AMD architecture for ROCm compatibility.
* `__GL_RENDERER_STRING` & `__GL_VENDOR_STRING`: Masks the OpenGL hardware identity.
* `RADV_FORCE_VND_ID` & `RADV_FORCE_DEV_ID`: Overrides Vulkan vendor and device IDs.

## Disclaimer
This project is for educational and developmental purposes. Hardware spoofing may violate the terms of service of certain proprietary applications. Use responsibly.



---

##  Ghost-Wrapper: Support the Grind

This tool exists because we shouldn't have to pay the "NVIDIA Tax" to run top-tier AI. If **Ghost-Wrapper** saved you from a 50-step Linux tutorial,consider supporting the project.

As a solo student dev, I keep things independent. No banks, no trackers—just code and caffeine.

### 💎 Digital Tokens (Direct Support)
| Asset | Address |
| :--- | :--- |
| **Ethereum (ETH)** | `0x9e046b3fa85932351b34520837d95bc1ad309748` |
| **Bitcoin (BTC)** | `bc1q30wftehky6ct06pshrjh3ekhe9as9uzzdemd0r` |

### 🌟 Social Support
* **Star this Repo:** It’s the best way to help the project grow.
* **Report a Bug**

*Keeping the RDNA dream alive, one wrapper at a time.*
