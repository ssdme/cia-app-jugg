# Rebuilding Sidecar Binaries

This guide documents the complete reproduction steps to build `beat_this` and `scenedetect` sidecars from upstream source.

---

# Part A: `beat_this` Sidecar

## 1. Upstream Source Repository & Commit

- **Repository**: [https://github.com/mosynthkey/beat_this_cpp](https://github.com/mosynthkey/beat_this_cpp)
- **Target Commit**: `07ab790a9ec2eda8093d52d249e3ec4f0510ee72`

---

## 2. Clone and Submodule Setup

```powershell
# Clone the repository
git clone https://github.com/mosynthkey/beat_this_cpp
cd beat_this_cpp
git checkout 07ab790a9ec2eda8093d52d249e3ec4f0510ee72

# Shallow clone the required C++ submodules (HTTPS)
git clone --depth 1 https://github.com/mackron/miniaudio.git Submodule/miniaudio
git clone --depth 1 https://github.com/mreineck/pocketfft.git Submodule/pocketfft
git clone --depth 1 https://github.com/avaneev/r8brain-free-src.git Submodule/r8brain
```

---

## 3. Apply Patch

Apply the patch `vendor/beat_this_cpp.patch` located in `cia-app-jugg`:

```powershell
# From the beat_this_cpp directory:
git apply path/to/cia-app-jugg/vendor/beat_this_cpp.patch
```

### Changes in `beat_this_cpp.patch`:
1. `Source/DBNPostprocessor.h`: Added `#include <array>` for MSVC standard library compatibility.
2. `Source/beat_this_api.h`: Added default argument `bool use_dbn = true` to `BeatThis` constructor declaration matching its definition.
3. `Source/main.cpp`: Added `--json` command-line switch to output machine-readable JSON:
   ```json
   {
     "bpm": 83.33,
     "beats": [0.4200, 1.1400, ...],
     "downbeats": [2.6000, 5.5000, ...]
   }
   ```

---

## 4. CMake Configuration and Compilation

Prerequisites: Visual Studio 2022/2026 with C++ desktop workload and CMake.

```powershell
# Configure Release build (CMake will automatically fetch ONNX Runtime 1.18.0)
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release

# Compile Release binaries
cmake --build build --config Release
```

---

## 5. Artifacts & Deployment

The compilation produces the following binaries in `build/Release`:

- `build/Release/beat_this_cpp.exe` $\rightarrow$ Copied as `src-tauri/binaries/beat_this.exe`
- `build/Release/beat_this_api.dll` $\rightarrow$ Copied as `src-tauri/binaries/beat_this_api.dll`
- `build/Release/onnxruntime.dll` $\rightarrow$ Copied as `src-tauri/binaries/onnxruntime.dll`
- `onnx/beat_this.onnx` $\rightarrow$ Copied as `src-tauri/binaries/beat_this.onnx`

All 4 files must reside together in `src-tauri/binaries/` for runtime execution.

---

# Part B: `scenedetect` Sidecar

## 1. Upstream Dependencies & Script

- **Library**: `scenedetect` (v0.7.1+) with `opencv-python`
- **Source CLI Script**: `vendor/scenedetect_cli.py`

## 2. Environment Setup

Prerequisites: Python 3.10+ (64-bit).

```powershell
# Install PySceneDetect, OpenCV, and PyInstaller
pip install "scenedetect" opencv-python pyinstaller
```

## 3. PyInstaller Standalone Compilation

```powershell
# From the cia-app-jugg repository root:
pyinstaller --onefile --clean --name scenedetect vendor/scenedetect_cli.py
```

## 4. Artifact Deployment

The compilation produces `dist/scenedetect.exe`:

- `dist/scenedetect.exe` $\rightarrow$ Copied as `src-tauri/binaries/scenedetect.exe`

Temporary directories `build/`, `dist/`, and `scenedetect.spec` may then be deleted.
