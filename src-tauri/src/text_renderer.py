import sys
import os
import json
import math
import subprocess
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageChops
import numpy as np

def find_coeffs(pa, pb):
    matrix = []
    for p1, p2 in zip(pa, pb):
        matrix.append([p1[0], p1[1], 1, 0, 0, 0, -p2[0]*p1[0], -p2[0]*p1[1]])
        matrix.append([0, 0, 0, p1[0], p1[1], 1, -p2[1]*p1[0], -p2[1]*p1[1]])
    A = np.matrix(matrix, dtype=float)
    B = np.array(pb).reshape(8)
    res = np.dot(np.linalg.inv(A.T * A) * A.T, B)
    return np.array(res).reshape(8)

def hex_to_rgb(hex_str, default=(255, 255, 255)):
    if not hex_str:
        return default
    h = hex_str.lstrip("#")
    if len(h) == 6:
        try:
            return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))
        except ValueError:
            return default
    elif len(h) == 8:
        try:
            return (int(h[2:4], 16), int(h[4:6], 16), int(h[6:8], 16))
        except ValueError:
            return default
    return default

def get_font(key, size, font_bahn, font_arial_bd, font_segoe_l, font_arial):
    try:
        if key == "hero" and font_bahn:
            return ImageFont.truetype(font_bahn, int(size))
        elif key == "bold" and font_arial_bd:
            return ImageFont.truetype(font_arial_bd, int(size))
        elif key == "light" and font_segoe_l:
            return ImageFont.truetype(font_segoe_l, int(size))
        elif font_arial:
            return ImageFont.truetype(font_arial, int(size))
    except Exception:
        pass
    return ImageFont.load_default()

def fast_glow(image_rgb, downscale=4, blur_radius=8):
    w, h = image_rgb.size
    small_w, small_h = max(1, w // downscale), max(1, h // downscale)
    small = image_rgb.resize((small_w, small_h), Image.Resampling.BILINEAR)
    blurred = small.filter(ImageFilter.GaussianBlur(blur_radius))
    return blurred.resize((w, h), Image.Resampling.BILINEAR)

# Rapid-word 4-frame kinetic scaling
RAPID_WORD_SCALES = [
    (1.0, 8.0),   # Frame 0: 8x vertical
    (8.0, 1.0),   # Frame 1: 8x horizontal
    (1.0, 6.0),   # Frame 2: 6x vertical
    (6.0, 1.0),   # Frame 3: 6x horizontal
]

def run_render(job_json_path):
    if not os.path.exists(job_json_path):
        print(f"Error: Job file not found at {job_json_path}")
        return

    with open(job_json_path, "r", encoding="utf-8") as f:
        job = json.load(f)

    audio_path = job.get("audioPath", "")
    style_method = job.get("styleMethod", "basic_effort")
    text_color_hex = job.get("textColor", "#FFFFFF" if style_method == "basic_effort" else "#FF0C14")
    glow_enabled = job.get("glowEnabled", True)
    glow_intensity = float(job.get("glowIntensity", 0.85))
    rapid_word = job.get("rapidWordEnabled", True)
    blocks = job.get("blocks", [])

    text_rgb = hex_to_rgb(text_color_hex, (255, 255, 255) if style_method == "basic_effort" else (255, 12, 20))

    downloads_dir = r"C:\Users\cia\Downloads"
    out_video = job.get("outputPath")
    if not out_video:
        out_video = os.path.join(downloads_dir, f"test_style_{style_method}_freestyle.mp4")

    # Dimensions and FPS
    W, H = 1080, 1080
    FPS = 60

    # Total duration from blocks
    if blocks:
        max_end = max([float(b.get("end", 0.0)) for b in blocks])
        duration = max(3.0, max_end + 0.5)
    else:
        duration = 10.0
    total_frames = int(FPS * duration)

    # Windows Fonts Resolution with Fail-Safe Fallbacks
    win_fonts = os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "Fonts")
    def resolve_font(*names):
        for name in names:
            candidate = os.path.join(win_fonts, name)
            if os.path.exists(candidate):
                return candidate
        return None

    font_bahn = resolve_font("bahnschrift.ttf", "arialbd.ttf", "arial.ttf")
    font_arial_bd = resolve_font("arialbd.ttf", "bahnschrift.ttf", "arial.ttf")
    font_segoe_l = resolve_font("segoeuil.ttf", "segoeui.ttf", "arial.ttf")
    font_arial = resolve_font("arial.ttf")
    font_mingliu = resolve_font("mingliub.ttc", "mingliu.ttc", "arialbd.ttf", "bahnschrift.ttf")

    ffmpeg_bin = job.get("ffmpegBin") or r"C:\Users\cia\scoop\shims\ffmpeg.exe"
    if not os.path.exists(ffmpeg_bin):
        ffmpeg_bin = "ffmpeg"

    # Pre-generate grain textures (30 frames cache)
    np.random.seed(42)
    grain_cache = []
    for _ in range(30):
        noise = np.random.normal(loc=1.0, scale=0.045, size=(H, W)).astype(np.float32)
        noise = np.clip(noise, 0.82, 1.18)
        img_l = Image.fromarray((noise * 255.0 / 1.18).astype(np.uint8), mode="L")
        grain_cache.append(Image.merge("RGB", (img_l, img_l, img_l)))

    # Low Effort Perspective Corner Pin parameters
    BUF_W, BUF_H = 750, 400
    dest_quad = [
        (520, 110),   # Top-Left
        (1020, 75),   # Top-Right (perspective tilt)
        (1015, 380),  # Bottom-Right
        (525, 430)    # Bottom-Left
    ]
    src_quad = [
        (0, 0),
        (BUF_W, 0),
        (BUF_W, BUF_H),
        (0, BUF_H)
    ]
    cornerpin_coeffs = find_coeffs(dest_quad, src_quad)
    low_font = ImageFont.truetype(font_mingliu, 42) if font_mingliu else ImageFont.load_default()

    # Pre-format 2-line verses for Low Effort with Word Timestamps
    verses = []
    if style_method == "low_effort":
        for b in blocks:
            raw_words = b.get("words", [])
            if raw_words and isinstance(raw_words[0], dict) and "word" in raw_words[0]:
                word_objs = raw_words
                words_text = [w["word"] for w in word_objs]
            else:
                words_text = b.get("wordsRef", [])
                if not words_text and b.get("elements"):
                    words_text = [w for el in b["elements"] for w in el.get("text", "").split()]
                word_objs = []

            if not words_text:
                continue

            mid = max(1, len(words_text) // 2)
            l1_words = words_text[:mid]
            l2_words = words_text[mid:]

            b_start = float(b.get("start", 0.0))
            b_end = float(b.get("end", 0.0))

            l1_objs = word_objs[:mid] if word_objs else []
            l2_objs = word_objs[mid:] if word_objs else []

            b_mid_t = float(l2_objs[0]["start"]) if (l2_objs and "start" in l2_objs[0]) else (b_start + (b_end - b_start) * 0.45)

            verses.append({
                "start": b_start,
                "end": b_end,
                "line1": {"text": " ".join(l1_words), "words": l1_words, "word_objs": l1_objs, "start": b_start, "end": b_mid_t},
                "line2": {"text": " ".join(l2_words), "words": l2_words, "word_objs": l2_objs, "start": b_mid_t, "end": b_end}
            })

    # Start FFmpeg pipe
    cmd = [
        ffmpeg_bin, "-y",
        "-f", "rawvideo",
        "-vcodec", "rawvideo",
        "-s", f"{W}x{H}",
        "-pix_fmt", "rgb24",
        "-r", str(FPS),
        "-i", "-",
    ]
    if audio_path and os.path.exists(audio_path):
        cmd.extend(["-i", audio_path])
    
    cmd.extend([
        "-c:v", "libx264",
        "-preset", "veryfast",
        "-crf", "16",
        "-pix_fmt", "yuv420p",
    ])
    if audio_path and os.path.exists(audio_path):
        cmd.extend(["-c:a", "aac", "-b:a", "320k", "-shortest"])

    cmd.append(out_video)

    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def get_revealed_line(line_info, cur_time):
        if cur_time < line_info["start"]:
            return ""
        if cur_time >= line_info["end"]:
            return line_info["text"]
        
        word_objs = line_info.get("word_objs")
        if word_objs and len(word_objs) == len(line_info["words"]):
            rev = []
            for w in word_objs:
                if cur_time >= float(w.get("start", line_info["start"])):
                    rev.append(w["word"])
            return " ".join(rev)

        words = line_info["words"]
        if not words:
            return ""
        step = (line_info["end"] - line_info["start"]) / max(1, len(words))
        active_w_count = min(len(words), int((cur_time - line_info["start"]) / max(0.04, step)) + 1)
        return " ".join(words[:active_w_count])

    for frame_idx in range(total_frames):
        t = frame_idx / FPS
        frame_canvas = Image.new("RGB", (W, H), (0, 0, 0))

        if style_method == "low_effort":
            # LOW EFFORT: Top-Right Perspective Corner Pin with 3.2x Vertical Stretch
            for verse in verses:
                v_start = verse["start"] - 0.05
                v_end = verse["end"] + 0.10
                if v_start <= t <= v_end:
                    alpha = 1.0
                    if t < verse["start"]:
                        alpha = max(0.0, (t - v_start) / 0.05)
                    elif t > verse["end"]:
                        alpha = max(0.0, 1.0 - (t - verse["end"]) / 0.10)

                    t1 = get_revealed_line(verse["line1"], t)
                    t2 = get_revealed_line(verse["line2"], t)

                    text_buf = Image.new("RGBA", (BUF_W, BUF_H), (0, 0, 0, 0))
                    col = (int(text_rgb[0] * alpha), int(text_rgb[1] * alpha), int(text_rgb[2] * alpha), int(255 * alpha))

                    line_spacing = 150
                    pad = 16
                    # Draw Line 1
                    if t1:
                        dummy = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
                        bb1 = dummy.textbbox((0, 0), t1, font=low_font)
                        w1, h1 = bb1[2] - bb1[0], bb1[3] - bb1[1]
                        img1 = Image.new("RGBA", (w1 + pad * 2, h1 + pad * 2), (0, 0, 0, 0))
                        ImageDraw.Draw(img1).text((pad - bb1[0], pad - bb1[1]), t1, font=low_font, fill=col)
                        
                        nw1 = int(img1.width * 0.95)
                        nh1 = int(img1.height * 3.2)
                        st1 = img1.resize((nw1, nh1), Image.Resampling.BILINEAR)
                        text_buf.paste(st1, (0, int(BUF_H // 2 - line_spacing / 2.0 - nh1 / 2.0)), st1)

                    # Draw Line 2
                    if t2:
                        dummy = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
                        bb2 = dummy.textbbox((0, 0), t2, font=low_font)
                        w2, h2 = bb2[2] - bb2[0], bb2[3] - bb2[1]
                        img2 = Image.new("RGBA", (w2 + pad * 2, h2 + pad * 2), (0, 0, 0, 0))
                        ImageDraw.Draw(img2).text((pad - bb2[0], pad - bb2[1]), t2, font=low_font, fill=col)
                        
                        nw2 = int(img2.width * 0.95)
                        nh2 = int(img2.height * 3.2)
                        st2 = img2.resize((nw2, nh2), Image.Resampling.BILINEAR)
                        text_buf.paste(st2, (0, int(BUF_H // 2 + line_spacing / 2.0 - nh2 / 2.0)), st2)

                    # Perspective Quad Warp with PERSPECTIVE transform
                    warped = text_buf.transform((W, H), Image.Transform.PERSPECTIVE, cornerpin_coeffs, Image.Resampling.BILINEAR)

                    # Optical red glow
                    glow_layer = warped.convert("RGB")
                    g1 = fast_glow(glow_layer, downscale=4, blur_radius=6)
                    g2 = fast_glow(glow_layer, downscale=8, blur_radius=14)
                    glow_comp = ImageChops.add(g1, g2)
                    frame_canvas = ImageChops.add(frame_canvas, glow_comp)

                    frame_canvas.paste(warped.convert("RGB"), (0, 0), mask=warped.split()[3])

        else:
            # BASIC EFFORT: 1:1 Puzzle Layout + Pro-Mist Glow + Rapid-Word
            current_block = None
            for b in blocks:
                if b.get("start", 0) <= t <= b.get("end", 0):
                    current_block = b
                    break

            if current_block:
                elements = current_block.get("elements", [])
                b_start = current_block.get("start", 0.0)
                b_end = current_block.get("end", 0.0)
                b_dur = max(0.1, b_end - b_start)

                text_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
                num_els = max(1, len(elements))
                step_dur = b_dur / num_els

                for e_idx, el in enumerate(elements):
                    el_start = el.get("start", b_start + e_idx * step_dur)
                    if t < el_start:
                        continue

                    fnt = get_font(el.get("key", "hero"), el.get("size", 100), font_bahn, font_arial_bd, font_segoe_l, font_arial)
                    txt = el.get("text", "")
                    target_x = int(el.get("x", 200))
                    target_y = int(el.get("y", 400))

                    # Bounding box
                    dummy_draw = ImageDraw.Draw(text_layer)
                    bbox = dummy_draw.textbbox((0, 0), txt, font=fnt)
                    bw = max(1, bbox[2] - bbox[0] + 30)
                    bh = max(1, bbox[3] - bbox[1] + 30)

                    # Rapid-Word Entrance (4 frames)
                    word_age_frames = int(round((t - el_start) * FPS))
                    if rapid_word and 0 <= word_age_frames < 4:
                        sx, sy = RAPID_WORD_SCALES[word_age_frames]
                        sticker = Image.new("RGBA", (bw + 20, bh + 20), (0, 0, 0, 0))
                        s_draw = ImageDraw.Draw(sticker)
                        s_draw.text((10 - bbox[0], 10 - bbox[1]), txt, font=fnt, fill=(*text_rgb, 255))

                        new_w = max(4, int((bw + 20) * sx))
                        new_h = max(4, int((bh + 20) * sy))
                        scaled_sticker = sticker.resize((new_w, new_h), Image.Resampling.BILINEAR)

                        cx = target_x + bw // 2
                        cy = target_y + bh // 2
                        paste_x = cx - new_w // 2
                        paste_y = cy - new_h // 2
                        text_layer.paste(scaled_sticker, (paste_x, paste_y), mask=scaled_sticker.split()[3])
                    else:
                        dummy_draw.text((target_x, target_y), txt, font=fnt, fill=(*text_rgb, 255))

                # Pro-Mist Optical Glow
                if glow_enabled:
                    glow_rgb = text_layer.convert("RGB")
                    g1 = fast_glow(glow_rgb, downscale=4, blur_radius=6)
                    g2 = fast_glow(glow_rgb, downscale=8, blur_radius=16)
                    composite_glow = ImageChops.add(g1, g2)
                    composite_glow = ImageChops.multiply(composite_glow, Image.new("RGB", (W, H), (int(255*glow_intensity), int(255*glow_intensity), int(255*glow_intensity))))
                    frame_canvas = ImageChops.add(frame_canvas, composite_glow)

                frame_canvas.paste(text_layer.convert("RGB"), (0, 0), mask=text_layer.split()[3])

        # 35mm Grain Overlay
        grain = grain_cache[frame_idx % len(grain_cache)]
        frame_canvas = ImageChops.multiply(frame_canvas, grain)

        # Write frame to FFmpeg pipe
        proc.stdin.write(frame_canvas.tobytes())

        # Progress reporting
        if frame_idx % 30 == 0 or frame_idx == total_frames - 1:
            pct = int((frame_idx + 1) / total_frames * 100)
            print(f"PROGRESS:{pct}:{frame_idx + 1}:{total_frames}", flush=True)

    proc.stdin.close()
    proc.wait()
    print("FINISHED", flush=True)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        run_render(sys.argv[1])
    else:
        print("Error: No job path provided")
