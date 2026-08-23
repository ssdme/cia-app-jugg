#!/usr/bin/env python3
"""
See-Through CLI: Semantic Character Layer Decomposition
Decomposes a single transparent character PNG into semantic layers (hair, face, eyes, clothes, etc.)
with stacking z_order metadata exported as layers.json.
"""

import sys
import os
import argparse
import json
from PIL import Image
import numpy as np

SEMANTIC_LAYERS = [
    {"name": "hair_back", "file": "hair_back.png", "zOrder": 0},
    {"name": "body", "file": "body.png", "zOrder": 1},
    {"name": "clothes_lower", "file": "clothes_lower.png", "zOrder": 2},
    {"name": "clothes_upper", "file": "clothes_upper.png", "zOrder": 3},
    {"name": "face", "file": "face.png", "zOrder": 4},
    {"name": "mouth", "file": "mouth.png", "zOrder": 5},
    {"name": "eyes", "file": "eyes.png", "zOrder": 6},
    {"name": "hair_front", "file": "hair_front.png", "zOrder": 7},
    {"name": "accessories", "file": "accessories.png", "zOrder": 8},
]


def decompose_character(input_path: str, output_dir: str):
    if not os.path.exists(input_path):
        raise FileNotFoundError(f"Input character PNG not found: {input_path}")

    os.makedirs(output_dir, exist_ok=True)

    img = Image.open(input_path).convert("RGBA")
    w, h = img.size
    img_arr = np.array(img, dtype=np.uint8)

    # Extract channels
    r = img_arr[:, :, 0].astype(np.float32)
    g = img_arr[:, :, 1].astype(np.float32)
    b = img_arr[:, :, 2].astype(np.float32)
    alpha = img_arr[:, :, 3]

    opaque_mask = alpha > 10

    # Grid coordinates normalized to [0, 1]
    y_coords, x_coords = np.mgrid[0:h, 0:w]
    y_norm = y_coords.astype(np.float32) / max(h - 1, 1)
    x_norm = x_coords.astype(np.float32) / max(w - 1, 1)

    # Color heuristics in RGB/HSV-like space
    # Skin tone detection
    is_skin = (
        (r > 130) & (g > 80) & (b > 60) &
        (r > g) & (g >= b) & ((r - b) > 15) &
        (y_norm < 0.70)
    )

    # Eyes/Mouth detection (head area y < 0.40, centered x in [0.25, 0.75])
    is_head_region = (y_norm < 0.42) & (x_norm > 0.20) & (x_norm < 0.80)
    is_eyes = is_head_region & ((r < 70) & (g < 70) & (b < 70) | ((r > 200) & (g > 200) & (b > 200))) & (y_norm > 0.15) & (y_norm < 0.32)
    is_mouth = is_head_region & (r > 150) & (g < 100) & (b < 110) & (y_norm >= 0.30) & (y_norm < 0.40)

    # Hair detection (top or extreme sides of head)
    is_hair_top = (y_norm < 0.28) & ~is_eyes
    is_hair_front = is_hair_top & (y_norm > 0.08) & (y_norm < 0.25)
    is_hair_back = is_hair_top & ~is_hair_front

    # Face base
    is_face = is_head_region & ~is_eyes & ~is_mouth & ~is_hair_top & is_skin

    # Clothes upper (torso y in [0.35, 0.70])
    is_torso = (y_norm >= 0.35) & (y_norm < 0.72) & ~is_skin
    # Clothes lower (legs y >= 0.70)
    is_lower = (y_norm >= 0.70)

    # Accessories (saturated or distinct contrast patches)
    is_acc = (r > 210) & (g > 180) & (b < 80)

    # Assign non-overlapping primary layer masks
    assigned = np.zeros((h, w), dtype=bool)
    layer_masks = {}

    def assign_layer(name, condition):
        nonlocal assigned
        mask = opaque_mask & condition & ~assigned
        assigned |= mask
        layer_masks[name] = mask

    assign_layer("eyes", is_eyes)
    assign_layer("mouth", is_mouth)
    assign_layer("face", is_face)
    assign_layer("accessories", is_acc)
    assign_layer("hair_front", is_hair_front)
    assign_layer("clothes_upper", is_torso)
    assign_layer("clothes_lower", is_lower)
    assign_layer("hair_back", is_hair_back)
    # Remaining opaque pixels assigned to body to guarantee complete, zero-loss coverage
    assign_layer("body", opaque_mask)

    layers_meta = []
    for item in SEMANTIC_LAYERS:
        name = item["name"]
        filename = item["file"]
        z_order = item["zOrder"]
        mask = layer_masks.get(name, np.zeros((h, w), dtype=bool))

        layer_img_arr = np.zeros((h, w, 4), dtype=np.uint8)
        layer_img_arr[mask] = img_arr[mask]

        layer_img = Image.fromarray(layer_img_arr, "RGBA")
        out_file_path = os.path.join(output_dir, filename)
        layer_img.save(out_file_path, "PNG")

        layers_meta.append({
            "name": name,
            "file": filename,
            "zOrder": z_order,
            "hasContent": bool(np.any(mask))
        })

    # Save layers.json
    layers_json_path = os.path.join(output_dir, "layers.json")
    with open(layers_json_path, "w", encoding="utf-8") as f:
        json.dump(layers_meta, f, indent=2)

    result = {
        "status": "success",
        "characterPath": input_path,
        "outputDir": output_dir,
        "layersCount": len(layers_meta),
        "layersJsonPath": layers_json_path,
        "layers": layers_meta,
    }
    return result


def main():
    parser = argparse.ArgumentParser(description="See-through Character Semantic Layer Decomposition CLI")
    parser.add_argument("--input", required=True, help="Path to transparent character PNG")
    parser.add_argument("--output-dir", required=True, help="Directory to write layer PNGs and layers.json")
    args = parser.parse_args()

    try:
        res = decompose_character(args.input, args.output_dir)
        print(json.dumps(res, indent=2))
        sys.exit(0)
    except Exception as e:
        err_res = {"status": "error", "message": str(e)}
        print(json.dumps(err_res), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
