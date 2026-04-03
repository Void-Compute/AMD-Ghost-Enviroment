# AMD Ghost Environment

## Overview
The AMD Ghost Environment is a lightweight, environment-level wrapper designed to facilitate compatibility between CUDA-centric software and AMD ROCm hardware. By injecting a targeted set of environment variables at runtime, this tool allows an AMD RDNA3 GPU to report its identity as an NVIDIA RTX 4090.

This prevents hard-coded architecture checks from blocking execution and ensures that libraries like PyTorch and ONNX Runtime route their workloads through the AMD ROCm compute layer via the `HSA_OVERRIDE_GFX_VERSION` mask.

## Key Features
* **Hardware Spoofing:** Overrides OpenGL and Vulkan renderer strings to bypass basic hardware gatekeeping.
* **Compute Hijacking:** Injects the necessary ROCm architecture masks to execute CUDA instructions on RDNA3 hardware.
* **Non-Destructive Integration:** Operates purely as a command-line wrapper. It does not permanently modify system files, bash profiles, or global states.
* **Virtual Environment Support:** Fully compatible with Python `venv`, Conda, and isolated Docker containers.

## Installation

1. Clone the repository to your local machine:
```bash
git clone [https://github.com/YOUR_USERNAME/AMD-Ghost-Environment.git](https://github.com/YOUR_USERNAME/AMD-Ghost-Environment.git)
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
ghost pip install torch torchvision torchaudio --index-url [https://download.pytorch.org/whl/rocm6.2](https://download.pytorch.org/whl/rocm6.2)
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
* `ROCm Version: 6.2` (or your current ROCm version)

## Technical Architecture
The `ghost` script utilizes the following primary variables to achieve hardware parity:
* `HSA_OVERRIDE_GFX_VERSION`: Defines the target AMD architecture for ROCm compatibility.
* `__GL_RENDERER_STRING` & `__GL_VENDOR_STRING`: Masks the OpenGL hardware identity.
* `RADV_FORCE_VND_ID` & `RADV_FORCE_DEV_ID`: Overrides Vulkan vendor and device IDs.

## Disclaimer
This project is for educational and developmental purposes. Hardware spoofing may violate the terms of service of certain proprietary applications. Use responsibly.
