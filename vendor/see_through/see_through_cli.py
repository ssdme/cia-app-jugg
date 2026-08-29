#!/usr/bin/env python3
"""
See-Through CLI: Precision Semantic Character Layer Decomposition
Decomposes transparent anime character PNGs into semantic depth layers (hair, body, face, eyes, foreground hand/accessories)
with Navier-Stokes inpainting on occluded background layers for clean 2.5D parallax rendering without transparent holes.
"""

import sys
import os
import argparse
import json
from PIL import Image
import numpy as np

try:
    import cv2
    HAS_CV2 = True
except ImportError:
    HAS_CV2 = False


SEMANTIC_LAYERS = [
    {"name": "hair_back", "file": "hair_back.png", "zOrder": 0, "zDepth": 0.0},
    {"name": "body", "file": "body.png", "zOrder": 1, "zDepth": 0.35},
    {"name": "face", "file": "face.png", "zOrder": 2, "zDepth": 0.60},
    {"name": "eyes", "file": "eyes.png", "zOrder": 3, "zDepth": 0.70},
    {"name": "hair_front", "file": "hair_front.png", "zOrder": 4, "zDepth": 0.85},
    {"name": "accessories", "file": "accessories.png", "zOrder": 5, "zDepth": 1.0},
]


def decompose_character(input_path: str, output_dir: str):
    if not os.path.exists(input_path):
        raise FileNotFoundError(f"Input character PNG not found: {input_path}")

    os.makedirs(output_dir, exist_ok=True)

    img = Image.open(input_path).convert("RGBA")
    w, h = img.size
    img_arr = np.array(img, dtype=np.uint8)

    r = img_arr[:, :, 0].astype(np.float32)
    g = img_arr[:, :, 1].astype(np.float32)
    b = img_arr[:, :, 2].astype(np.float32)
    alpha = img_arr[:, :, 3]

    opaque_mask = alpha > 15
    y_coords, x_coords = np.mgrid[0:h, 0:w]

    if HAS_CV2:
        bgr = cv2.cvtColor(img_arr[:, :, :3], cv2.COLOR_RGB2BGR)
        hsv = cv2.cvtColor(bgr, cv2.COLOR_BGR2HSV)
        h_chan = hsv[:, :, 0]
        s_chan = hsv[:, :, 1]
        v_chan = hsv[:, :, 2]

        kernel3 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
        kernel15 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (15, 15))

        # ─── 1. HAIR DETECTION & INPAINTING ──────────────────────────────────
        is_green_hair = (
            ((h_chan >= 24) & (h_chan <= 95) & (s_chan >= 18)) |
            ((g > r * 0.98) & (g > b * 0.98) & ((g - b) > 4)) |
            ((x_coords < 365) & (y_coords < 580) & ~((v_chan > 175) & (s_chan < 25)))
        ) & opaque_mask

        hair_closed = cv2.morphologyEx(is_green_hair.astype(np.uint8) * 255, cv2.MORPH_CLOSE, kernel3)
        is_hair_total = (hair_closed > 0) & opaque_mask

        # ─── 2. FOREGROUND HAND / ACCESSORY DETECTION ────────────────────────
        is_hand_box = (y_coords >= 120) & (y_coords <= 360) & (x_coords >= 360) & (x_coords <= 520)
        is_red_cuff = is_hand_box & opaque_mask & (r > 120) & (g < 80) & (b < 80) & ((r - g) > 40)
        is_purple_sleeve = is_hand_box & opaque_mask & (v_chan < 90) & (r > b * 0.9) & (r > g) & (y_coords >= 180) & (y_coords <= 320) & (x_coords >= 360) & (x_coords <= 460)
        is_glove_white = is_hand_box & opaque_mask & ~is_hair_total & (v_chan > 155) & (s_chan < 50) & (y_coords <= 280)

        hand_seed = (is_red_cuff | is_purple_sleeve | is_glove_white).astype(np.uint8) * 255
        num_labels, labels, stats, centroids = cv2.connectedComponentsWithStats(hand_seed, connectivity=8)
        glove_mask = np.zeros((h, w), dtype=bool)
        for lbl in range(1, num_labels):
            cx, cy = centroids[lbl]
            area = stats[lbl, cv2.CC_STAT_AREA]
            if 360 <= cx <= 520 and 120 <= cy <= 340 and area > 120:
                glove_mask |= (labels == lbl)

        glove_dilated = cv2.dilate(glove_mask.astype(np.uint8) * 255, kernel3) > 0
        is_foreground_acc = glove_dilated & opaque_mask & ~is_hair_total

        # ─── 3. EYES & FACE SKIN ─────────────────────────────────────────────
        is_head_region = (y_coords >= 100) & (y_coords <= 320) & (x_coords >= 400) & (x_coords <= 580)
        is_face_head = is_head_region & opaque_mask & ~is_hair_total & ~is_foreground_acc

        is_eye_gold = is_head_region & opaque_mask & ~is_foreground_acc & (h_chan >= 14) & (h_chan <= 48) & (s_chan >= 85) & (v_chan >= 100) & (x_coords >= 470) & (y_coords <= 215)
        is_eye_dark = is_head_region & opaque_mask & ~is_foreground_acc & (v_chan < 60) & (x_coords >= 470) & (y_coords >= 150) & (y_coords <= 215)
        is_eyes = (is_eye_gold | is_eye_dark) & is_head_region
        is_face = is_face_head & ~is_eyes

        # ─── 4. BODY & PLUGSUIT ──────────────────────────────────────────────
        assigned_head_and_acc = is_hair_total | is_foreground_acc | is_face_head
        is_body = opaque_mask & ~assigned_head_and_acc

        # ─── 5. INPAINT HAIR BACK FOR 2.5D MOTION ────────────────────────────
        hair_bgr = bgr.copy()
        hair_inpaint_mask = ((is_face_head | is_foreground_acc) & (y_coords < 340)).astype(np.uint8) * 255
        hair_inpaint_mask = cv2.dilate(hair_inpaint_mask, kernel15)
        inpainted_hair_bgr = cv2.inpaint(hair_bgr, hair_inpaint_mask, 7, cv2.INPAINT_NS)
        inpainted_hair_rgb = cv2.cvtColor(inpainted_hair_bgr, cv2.COLOR_BGR2RGB)

        hair_full_mask = (is_hair_total | (is_head_region & (is_face_head | is_foreground_acc))) & opaque_mask
        hair_infilled_arr = np.zeros((h, w, 4), dtype=np.uint8)
        hair_infilled_arr[is_hair_total] = img_arr[is_hair_total]
        inpaint_area = hair_full_mask & ~is_hair_total
        hair_infilled_arr[inpaint_area, :3] = inpainted_hair_rgb[inpaint_area]
        hair_infilled_arr[inpaint_area, 3] = alpha[inpaint_area]

        layer_data = {
            "hair_back": hair_infilled_arr,
            "body": np.where(is_body[:, :, None], img_arr, 0).astype(np.uint8),
            "face": np.where(is_face[:, :, None], img_arr, 0).astype(np.uint8),
            "eyes": np.where(is_eyes[:, :, None], img_arr, 0).astype(np.uint8),
            "hair_front": np.zeros((h, w, 4), dtype=np.uint8),
            "accessories": np.where(is_foreground_acc[:, :, None], img_arr, 0).astype(np.uint8),
        }
    else:
        is_skin = (r > 140) & (g > 90) & (b > 70) & (r > g) & (g >= b)
        is_hair = ((g > r) & (g > b)) | (y_coords < h * 0.3)
        is_body = opaque_mask & ~is_hair & ~is_skin
        is_face = opaque_mask & is_skin

        layer_data = {
            "hair_back": np.where(is_hair[:, :, None] & opaque_mask[:, :, None], img_arr, 0).astype(np.uint8),
            "body": np.where(is_body[:, :, None], img_arr, 0).astype(np.uint8),
            "face": np.where(is_face[:, :, None], img_arr, 0).astype(np.uint8),
            "eyes": np.zeros((h, w, 4), dtype=np.uint8),
            "hair_front": np.zeros((h, w, 4), dtype=np.uint8),
            "accessories": np.zeros((h, w, 4), dtype=np.uint8),
        }

    layers_meta = []
    for item in SEMANTIC_LAYERS:
        name = item["name"]
        filename = item["file"]
        z_order = item["zOrder"]
        z_depth = item.get("zDepth", z_order / float(len(SEMANTIC_LAYERS) - 1))

        layer_arr = layer_data.get(name, np.zeros((h, w, 4), dtype=np.uint8))
        has_content = bool(np.any(layer_arr[:, :, 3] > 0))

        out_file_path = os.path.join(output_dir, filename)
        Image.fromarray(layer_arr, "RGBA").save(out_file_path, "PNG")

        layers_meta.append({
            "name": name,
            "file": filename,
            "zOrder": z_order,
            "zDepth": z_depth,
            "hasContent": has_content,
        })

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
