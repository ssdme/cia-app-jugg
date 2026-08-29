#!/usr/bin/env python3
"""
See-Through CLI: Precision Semantic Character Layer Decomposition
Decomposes transparent anime character PNGs (portraits and full-body figures) into semantic depth layers
(Hair with Navier-Stokes infilled backing, Body/Clothes, Face/Features, Foreground Hand/Accessories).
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

        # ─── 1. GREEN / ANIME HAIR DETECTION ─────────────────────────────────
        is_green_hue = (h_chan >= 22) & (h_chan <= 95) & (s_chan >= 16)
        is_green_dominant = (g > r * 0.96) & (g > b * 0.96) & ((g - b) > 3)
        is_left_strands = (x_coords < w * 0.45) & (y_coords < h * 0.85) & ~((v_chan > 175) & (s_chan < 25))
        is_hair_raw = opaque_mask & (is_green_hue | is_green_dominant | is_left_strands)

        hair_closed = cv2.morphologyEx(is_hair_raw.astype(np.uint8) * 255, cv2.MORPH_CLOSE, kernel3)
        is_hair = (hair_closed > 0) & opaque_mask

        # ─── 2. SKIN TONES & HANDS ───────────────────────────────────────────
        is_skin_tone = opaque_mask & ~is_hair & (
            (r > 145) & (g > 105) & (b > 75) &
            (r > g) & (g >= b) & ((r - b) > 10) &
            (h_chan <= 28) & (s_chan >= 10) & (s_chan <= 160)
        )

        num_skin_labels, skin_labels, skin_stats, skin_centroids = cv2.connectedComponentsWithStats(
            is_skin_tone.astype(np.uint8) * 255, connectivity=8
        )

        is_hand = np.zeros((h, w), dtype=bool)
        is_face_skin = np.zeros((h, w), dtype=bool)

        head_cy_threshold = h * 0.55
        for s_idx in range(1, num_skin_labels):
            comp = (skin_labels == s_idx)
            cx, cy = skin_centroids[s_idx]
            area = skin_stats[s_idx, cv2.CC_STAT_AREA]
            if area < 25:
                continue
            if cy < head_cy_threshold and cx > w * 0.45:
                is_face_skin |= comp
            elif cy >= head_cy_threshold * 0.7:
                is_hand |= comp
            else:
                is_face_skin |= comp

        # ─── 3. EYES & FACE FEATURES ─────────────────────────────────────────
        is_eye_box = is_face_skin | (opaque_mask & ~is_hair & (y_coords < head_cy_threshold) & (x_coords > w * 0.40))
        is_eye_gold = is_eye_box & (h_chan >= 12) & (h_chan <= 48) & (s_chan >= 60) & (v_chan >= 80)
        is_eye_dark = is_eye_box & (v_chan < 65)
        is_eyes = (is_eye_gold | is_eye_dark) & ~is_hair & ~is_hand
        is_face = (is_face_skin | is_eyes) & ~is_hair & ~is_hand

        # ─── 4. FOREGROUND ACCESSORIES / GLOVE / HAND ────────────────────────
        # Red cuffs / wrist accents if present
        is_red_acc = opaque_mask & (r > 120) & (g < 80) & (b < 80) & ((r - g) > 40)
        is_acc = (is_hand | is_red_acc) & ~is_hair

        # ─── 5. BODY & CLOTHES ───────────────────────────────────────────────
        assigned = is_hair | is_face | is_eyes | is_acc
        is_body = opaque_mask & ~assigned

        # ─── 6. INPAINT HAIR BACKING FOR SEAMLESS 2.5D MOTION ────────────────
        hair_bgr = bgr.copy()
        inpaint_mask = ((is_face | is_acc | is_body) & (x_coords > w * 0.35)).astype(np.uint8) * 255
        inpaint_mask = cv2.dilate(inpaint_mask, kernel15)
        inpainted_hair_bgr = cv2.inpaint(hair_bgr, inpaint_mask, 9, cv2.INPAINT_NS)
        inpainted_hair_rgb = cv2.cvtColor(inpainted_hair_bgr, cv2.COLOR_BGR2RGB)

        hair_out = np.zeros((h, w, 4), dtype=np.uint8)
        hair_out[is_hair] = img_arr[is_hair]
        inpaint_area = (inpaint_mask > 0) & opaque_mask & ~is_hair
        hair_out[inpaint_area, :3] = inpainted_hair_rgb[inpaint_area]
        hair_out[inpaint_area, 3] = alpha[inpaint_area]

        layer_data = {
            "hair_back": hair_out,
            "body": np.where(is_body[:, :, None], img_arr, 0).astype(np.uint8),
            "face": np.where(is_face[:, :, None], img_arr, 0).astype(np.uint8),
            "eyes": np.where(is_eyes[:, :, None], img_arr, 0).astype(np.uint8),
            "hair_front": np.zeros((h, w, 4), dtype=np.uint8),
            "accessories": np.where(is_acc[:, :, None], img_arr, 0).astype(np.uint8),
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
