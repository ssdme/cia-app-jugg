#!/usr/bin/env python3
"""
See-Through Sidecar Bootstrap Installer
Detects NVIDIA GPU, creates Python virtual environment, installs dependencies, and prepares sidecar CLI.
"""

import sys
import os
import subprocess
import shutil

def check_nvidia_gpu() -> bool:
    try:
        res = subprocess.run(["nvidia-smi"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        return res.returncode == 0
    except Exception:
        return False

def main():
    print("=== See-through Sidecar Bootstrap Setup ===")

    # 1. GPU Check
    has_gpu = check_nvidia_gpu()
    if not has_gpu:
        print("[ERROR] NVIDIA GPU with CUDA support was not detected on this machine.")
        print("See-through layer decomposition requires an NVIDIA GPU (RTX / GTX).")
        sys.exit(2)

    print("[OK] NVIDIA GPU detected.")

    # 2. Determine target venv directory
    app_data = os.getenv("LOCALAPPDATA", os.path.expanduser("~"))
    sidecar_dir = os.path.join(app_data, "cia_app", "sidecars", "see_through")
    venv_dir = os.path.join(sidecar_dir, "venv")
    os.makedirs(sidecar_dir, exist_ok=True)

    print(f"Target sidecar directory: {sidecar_dir}")

    # 3. Create venv if not exists
    python_exe = sys.executable
    if not os.path.exists(os.path.join(venv_dir, "Scripts", "python.exe")):
        print("Creating Python virtual environment...")
        subprocess.check_call([python_exe, "-m", "venv", venv_dir])

    venv_python = os.path.join(venv_dir, "Scripts", "python.exe")
    venv_pip = os.path.join(venv_dir, "Scripts", "pip.exe")

    # 4. Install dependencies
    print("Installing dependencies into virtual environment...")
    subprocess.check_call([venv_pip, "install", "--upgrade", "pip"])
    subprocess.check_call([venv_pip, "install", "pillow", "numpy", "opencv-python", "scipy"])

    print("\n[SUCCESS] See-through sidecar environment bootstrapped successfully!")
    print(f"Venv Python: {venv_python}")
    sys.exit(0)

if __name__ == "__main__":
    main()
