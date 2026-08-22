# See-through Sidecar Setup & Build Guide

This document outlines the setup, architecture, and deployment of the **See-through** character layer decomposition sidecar for `cia-app-jugg`.

---

## 1. Overview & Architecture

- **Upstream Repository**: [https://github.com/shitagaki-lab/see-through](https://github.com/shitagaki-lab/see-through)
- **Purpose**: Decompose a transparent anime/character PNG into semantically separated, inpainted RGBA layers (`hair_front`, `face`, `eyes`, `eyebrows`, `mouth`, `body`, `clothes_upper`, `clothes_lower`, `hair_back`, `accessories`) with an explicit `z_order` stacking sequence.
- **Hardware Requirement**: NVIDIA GPU with CUDA support (Compute Capability 6.0+, >=4GB VRAM). If no NVIDIA GPU is detected, the application surfaces an informational error card and does not crash.

---

## 2. Directory Structure

```text
vendor/see_through/
├── BUILD.md                  # This documentation file
├── bootstrap_see_through.py  # Automated setup & venv installer
└── see_through_cli.py        # Standalone CLI interface (input -> output dir + layers.json)
```

At runtime, the bootstrapped virtual environment is located at:
- `%LOCALAPPDATA%\cia_app\sidecars\see_through\venv\` (or `~/.cia/sidecars/see_through/venv/`)

---

## 3. Automated Bootstrap Installation

Run the bootstrap installer from PowerShell:

```powershell
python vendor/see_through/bootstrap_see_through.py
```

The bootstrap script performs:
1. **NVIDIA GPU Detection**: Executes `nvidia-smi` to verify hardware presence and driver readiness.
2. **Virtual Environment Setup**: Initializes a Python 3.11 virtual environment under the sidecars directory.
3. **PyTorch with CUDA**: Installs `torch` and `torchvision` with CUDA 12.1/12.4 wheels.
4. **Dependencies**: Installs `pillow`, `numpy`, `opencv-python`, `scipy`, `huggingface_hub`.
5. **Weights Download**: Fetches layer decomposition weights from HuggingFace (`layerdifforg/seethroughv0.0.2_layerdiff3d`).

---

## 4. CLI Execution Interface

The CLI entrypoint follows the standard sidecar interface:

```powershell
python vendor/see_through/see_through_cli.py --input <path_to_character.png> --output-dir <path_to_output_dir>
```

### Outputs:
- Individual layer PNGs: `hair_back.png`, `body.png`, `clothes_lower.png`, `clothes_upper.png`, `face.png`, `mouth.png`, `eyes.png`, `hair_front.png`, `accessories.png`.
- `layers.json`:
  ```json
  [
    { "name": "hair_back", "file": "hair_back.png", "zOrder": 0 },
    { "name": "body", "file": "body.png", "zOrder": 1 },
    { "name": "clothes_lower", "file": "clothes_lower.png", "zOrder": 2 },
    { "name": "clothes_upper", "file": "clothes_upper.png", "zOrder": 3 },
    { "name": "face", "file": "face.png", "zOrder": 4 },
    { "name": "mouth", "file": "mouth.png", "zOrder": 5 },
    { "name": "eyes", "file": "eyes.png", "zOrder": 6 },
    { "name": "hair_front", "file": "hair_front.png", "zOrder": 7 },
    { "name": "accessories", "file": "accessories.png", "zOrder": 8 }
  ]
  ```

### Recomposition Invariant:
Compositing the layers in `z_order` from 0 to N on a transparent canvas reconstructs the original character with color difference $\le 2$ per channel on all non-transparent pixels.
