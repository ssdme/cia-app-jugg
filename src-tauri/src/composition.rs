use std::path::{Path, PathBuf};
use tauri::Manager;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn encode_base64(data: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
        result.push(B64_CHARS[((n >> 12) & 63) as usize] as char);

        if chunk.len() > 1 {
            result.push(B64_CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(B64_CHARS[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayerItem {
    pub name: String,
    pub file: String,
    pub z_order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_content: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_base64: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionResult {
    pub status: String,
    pub character_path: String,
    pub background_path: Option<String>,
    pub output_dir: String,
    pub layers_count: usize,
    pub layers_json_path: String,
    pub layers: Vec<LayerItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompProject {
    pub schema_version: String, // "comp_project_v1"
    pub character_path: String,
    pub background_path: Option<String>,
    pub layers: Vec<LayerItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    Normal,
    Multiply,
    Add,
    Screen,
    Lighten,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionOp {
    pub id: String,
    pub name: String,
    pub op_type: String, // "drop_shadow" | "light_wrap" | "tint" | "rim_light" | "gradient" | "blur"
    pub blend_mode: BlendMode,
    pub opacity: f32, // 0.0 .. 1.0
    pub mask_by_alpha: bool,
    pub enabled: bool,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionProgress {
    pub phase: String, // "COMPOSITING" | "ENCODING" | "DONE"
    pub percent: u32,
    pub current_frame: u32,
    pub total_frames: u32,
    pub message: String,
}

pub fn get_default_composition_ops() -> Vec<CompositionOp> {
    vec![
        CompositionOp {
            id: "drop_shadow".to_string(),
            name: "Drop Shadow".to_string(),
            op_type: "drop_shadow".to_string(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.60,
            mask_by_alpha: false,
            enabled: true,
            params: serde_json::json!({
                "offsetX": 12.0,
                "offsetY": 16.0,
                "blurRadius": 14.0,
                "color": [0, 0, 0]
            }),
        },
        CompositionOp {
            id: "light_wrap".to_string(),
            name: "Light Wrap".to_string(),
            op_type: "light_wrap".to_string(),
            blend_mode: BlendMode::Screen,
            opacity: 0.55,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "blurRadius": 22.0,
                "edgeWidth": 10.0
            }),
        },
        CompositionOp {
            id: "tint_raccord".to_string(),
            name: "Tint de raccord".to_string(),
            op_type: "tint".to_string(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.07,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "mode": "auto_background"
            }),
        },
        CompositionOp {
            id: "rim_light".to_string(),
            name: "Rim Light".to_string(),
            op_type: "rim_light".to_string(),
            blend_mode: BlendMode::Add,
            opacity: 0.65,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "edgeWidth": 4.0,
                "color": [220, 240, 255]
            }),
        },
    ]
}

#[inline(always)]
pub fn apply_blend_mode(b: f32, s: f32, mode: &BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => s,
        BlendMode::Multiply => b * s,
        BlendMode::Add => (b + s).min(1.0),
        BlendMode::Screen => 1.0 - (1.0 - b) * (1.0 - s),
        BlendMode::Lighten => b.max(s),
    }
}

#[inline(always)]
pub fn alpha_over_pixel(bg: [u8; 4], fg: [u8; 4]) -> [u8; 4] {
    let a_fg = fg[3] as f32 / 255.0;
    if a_fg <= 0.0 {
        return bg;
    }
    let a_bg = bg[3] as f32 / 255.0;
    let a_out = a_fg + a_bg * (1.0 - a_fg);
    if a_out <= 0.0 {
        return [0, 0, 0, 0];
    }
    let r = (fg[0] as f32 * a_fg + bg[0] as f32 * a_bg * (1.0 - a_fg)) / a_out;
    let g = (fg[1] as f32 * a_fg + bg[1] as f32 * a_bg * (1.0 - a_fg)) / a_out;
    let b = (fg[2] as f32 * a_fg + bg[2] as f32 * a_bg * (1.0 - a_fg)) / a_out;

    [
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        (a_out * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

pub fn gaussian_blur_channel(src: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    if radius <= 0.5 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let sigma = radius;
    let k_radius = (sigma * 2.5).ceil() as i32;
    let mut kernel = Vec::with_capacity((k_radius * 2 + 1) as usize);
    let mut k_sum = 0.0f32;
    for i in -k_radius..=k_radius {
        let val = (-((i * i) as f32) / (2.0 * sigma * sigma)).exp();
        kernel.push(val);
        k_sum += val;
    }
    for k in &mut kernel {
        *k /= k_sum;
    }

    // Horizontal pass
    let mut temp = vec![0.0f32; w * h];
    for y in 0..h {
        let row_offset = y * w;
        for x in 0..w {
            let mut acc = 0.0f32;
            for (idx, &k_val) in kernel.iter().enumerate() {
                let offset = (idx as i32) - k_radius;
                let nx = (x as i32 + offset).clamp(0, w as i32 - 1) as usize;
                acc += src[row_offset + nx] * k_val;
            }
            temp[row_offset + x] = acc;
        }
    }

    // Vertical pass
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let row_offset = y * w;
        for x in 0..w {
            let mut acc = 0.0f32;
            for (idx, &k_val) in kernel.iter().enumerate() {
                let offset = (idx as i32) - k_radius;
                let ny = (y as i32 + offset).clamp(0, h as i32 - 1) as usize;
                acc += temp[ny * w + x] * k_val;
            }
            out[row_offset + x] = acc;
        }
    }
    out
}

pub fn extract_inner_edge_mask(alpha: &[f32], w: usize, h: usize, edge_radius: f32) -> Vec<f32> {
    let blurred = gaussian_blur_channel(alpha, w, h, edge_radius.max(2.0));
    let mut edge = vec![0.0f32; w * h];
    for i in 0..(w * h) {
        let a = alpha[i];
        if a > 0.05 {
            let diff = (a - blurred[i]).max(0.0);
            edge[i] = (diff * 2.8 * a).clamp(0.0, 1.0);
        } else {
            edge[i] = 0.0;
        }
    }
    edge
}

pub fn extract_contour_rim_mask(alpha: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut rim = vec![0.0f32; w * h];
    for y in 0..h {
        let y_prev = y.saturating_sub(1);
        let y_next = (y + 1).min(h - 1);
        for x in 0..w {
            let x_prev = x.saturating_sub(1);
            let x_next = (x + 1).min(w - 1);

            let a = alpha[y * w + x];
            if a > 0.05 {
                let dx = (alpha[y * w + x_next] - alpha[y * w + x_prev]) * 0.5;
                let dy = (alpha[y_next * w + x] - alpha[y_prev * w + x]) * 0.5;
                let mag = (dx * dx + dy * dy).sqrt();
                rim[y * w + x] = (mag * 3.5 * a).clamp(0.0, 1.0);
            } else {
                rim[y * w + x] = 0.0;
            }
        }
    }
    rim
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>, // RGBA (width * height * 4)
}

pub fn resize_bilinear_rgba(src: &RawImage, target_w: usize, target_h: usize) -> RawImage {
    if src.width == target_w && src.height == target_h {
        return src.clone();
    }
    let mut out_data = vec![0u8; target_w * target_h * 4];
    let x_ratio = if target_w > 1 { (src.width - 1) as f32 / (target_w - 1) as f32 } else { 0.0 };
    let y_ratio = if target_h > 1 { (src.height - 1) as f32 / (target_h - 1) as f32 } else { 0.0 };

    for y in 0..target_h {
        let sy = y as f32 * y_ratio;
        let y_low = sy.floor() as usize;
        let y_high = (y_low + 1).min(src.height - 1);
        let y_weight = sy - y_low as f32;

        let row_out = y * target_w * 4;
        let row_low = y_low * src.width * 4;
        let row_high = y_high * src.width * 4;

        for x in 0..target_w {
            let sx = x as f32 * x_ratio;
            let x_low = sx.floor() as usize;
            let x_high = (x_low + 1).min(src.width - 1);
            let x_weight = sx - x_low as f32;

            let idx_out = row_out + x * 4;
            let idx_00 = row_low + x_low * 4;
            let idx_10 = row_low + x_high * 4;
            let idx_01 = row_high + x_low * 4;
            let idx_11 = row_high + x_high * 4;

            for c in 0..4 {
                let top = src.data[idx_00 + c] as f32 * (1.0 - x_weight) + src.data[idx_10 + c] as f32 * x_weight;
                let bottom = src.data[idx_01 + c] as f32 * (1.0 - x_weight) + src.data[idx_11 + c] as f32 * x_weight;
                let val = top * (1.0 - y_weight) + bottom * y_weight;
                out_data[idx_out + c] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    RawImage {
        width: target_w,
        height: target_h,
        data: out_data,
    }
}

pub fn probe_image_dimensions(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<(usize, usize), String> {
    // 1. Try ffprobe
    let ffprobe_bin = if let Some(bin) = ffmpeg_bin {
        if let Some(parent) = bin.parent() {
            let direct = parent.join("ffprobe.exe");
            if direct.exists() {
                direct
            } else {
                PathBuf::from("ffprobe")
            }
        } else {
            PathBuf::from("ffprobe")
        }
    } else {
        PathBuf::from("ffprobe")
    };

    let mut probe_cmd = std::process::Command::new(&ffprobe_bin);
    probe_cmd.args([
        "-v", "error",
        "-select_streams", "v:0",
        "-show_entries", "stream=width,height",
        "-of", "csv=p=0:s=x",
        path.to_str().unwrap(),
    ]);
    #[cfg(target_os = "windows")]
    probe_cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(output) = probe_cmd.output() {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<&str> = out_str.split('x').collect();
            if parts.len() == 2 {
                if let (Ok(w), Ok(h)) = (parts[0].trim().parse::<usize>(), parts[1].trim().parse::<usize>()) {
                    if w > 0 && h > 0 {
                        return Ok((w, h));
                    }
                }
            }
        }
    }

    // 2. Fallback: Parse ffmpeg -i stderr
    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["-i", path.to_str().unwrap()]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    for line in stderr_str.lines() {
        if line.contains("Video:") {
            for token in line.split(',') {
                let tok = token.trim();
                let parts: Vec<&str> = tok.split('x').collect();
                if parts.len() == 2 {
                    let w_str = parts[0].trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                    let h_str = parts[1].trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                    if let (Ok(w), Ok(h)) = (w_str.parse::<usize>(), h_str.parse::<usize>()) {
                        if w > 0 && h > 0 {
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
    }

    Err(format!("Could not determine dimensions of image {}", path.display()))
}

pub fn load_image_rgba(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<RawImage, String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let (w, h) = probe_image_dimensions(path, ffmpeg_bin)?;

    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args([
        "-i", path.to_str().unwrap(),
        "-f", "rawvideo",
        "-pix_fmt", "rgba",
        "-",
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to decode image via ffmpeg: {e}"))?;
    if !output.status.success() || output.stdout.len() < w * h * 4 {
        return Err(format!("Failed to read raw RGBA data from {}", path.display()));
    }

    Ok(RawImage {
        width: w,
        height: h,
        data: output.stdout[..w * h * 4].to_vec(),
    })
}

pub fn save_image_rgba(img: &RawImage, path: &Path, ffmpeg_bin: Option<&Path>) -> Result<(), String> {
    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args([
        "-y",
        "-f", "rawvideo",
        "-pix_fmt", "rgba",
        "-s", &format!("{}x{}", img.width, img.height),
        "-i", "-",
        "-frames:v", "1",
        path.to_str().unwrap(),
    ]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn ffmpeg for image save: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(&img.data);
    }
    let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg save: {e}"))?;
    if !status.success() {
        return Err("Failed to save image via ffmpeg".to_string());
    }
    Ok(())
}

pub fn validate_and_load_character_png(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<RawImage, String> {
    let raw = load_image_rgba(path, ffmpeg_bin)?;

    // Check if there is actual alpha transparency
    let mut has_transparency = false;
    for i in 0..(raw.width * raw.height) {
        if raw.data[i * 4 + 3] < 250 {
            has_transparency = true;
            break;
        }
    }

    if !has_transparency {
        return Err("PNG sans canal alpha — détourage requis".to_string());
    }

    Ok(raw)
}

pub fn check_nvidia_gpu_internal() -> Result<String, String> {
    let mut cmd = std::process::Command::new("nvidia-smi");
    cmd.args(["--query-gpu=name,driver_version,memory.total", "--format=csv,noheader"]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !stdout.is_empty() {
                    Ok(stdout)
                } else {
                    Ok("NVIDIA GPU Detected".to_string())
                }
            } else {
                Err("NVIDIA GPU not detected. See-through layer decomposition requires an NVIDIA GPU with CUDA.".to_string())
            }
        }
        Err(e) => {
            Err(format!("NVIDIA GPU check failed: {e}. See-through requires an NVIDIA GPU with CUDA."))
        }
    }
}

fn find_see_through_cli_script() -> Option<PathBuf> {
    let direct = PathBuf::from("vendor").join("see_through").join("see_through_cli.py");
    if direct.exists() {
        return Some(direct);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let in_cwd = cwd.join("vendor").join("see_through").join("see_through_cli.py");
    if in_cwd.exists() {
        return Some(in_cwd);
    }
    if let Some(parent) = cwd.parent() {
        let in_parent = parent.join("vendor").join("see_through").join("see_through_cli.py");
        if in_parent.exists() {
            return Some(in_parent);
        }
    }
    if let Ok(exe_p) = std::env::current_exe() {
        let mut cur = exe_p.parent();
        while let Some(p) = cur {
            let in_p = p.join("vendor").join("see_through").join("see_through_cli.py");
            if in_p.exists() {
                return Some(in_p);
            }
            cur = p.parent();
        }
    }
    None
}

pub fn resolve_see_through_python(app: Option<&tauri::AppHandle>) -> Result<(PathBuf, PathBuf), String> {
    let cli_script = find_see_through_cli_script().ok_or_else(|| {
        "vendor/see_through/see_through_cli.py not found in project or vendor directory.".to_string()
    })?;

    // 1. Check venv in %LOCALAPPDATA%/cia_app/sidecars/see_through/venv
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let venv_py = PathBuf::from(&local_app_data)
            .join("cia_app")
            .join("sidecars")
            .join("see_through")
            .join("venv")
            .join("Scripts")
            .join("python.exe");

        if venv_py.exists() {
            return Ok((venv_py, cli_script));
        }
    }

    // 2. Check app data dir if tauri app is available
    if let Some(app_handle) = app {
        if let Ok(app_dir) = app_handle.path().app_data_dir() {
            let venv_py = app_dir
                .join("sidecars")
                .join("see_through")
                .join("venv")
                .join("Scripts")
                .join("python.exe");

            if venv_py.exists() {
                return Ok((venv_py, cli_script));
            }
        }
    }

    // 3. Fallback: check system python
    let candidates = ["py", "python", "python3"];
    for cand in candidates {
        let mut check_cmd = std::process::Command::new(cand);
        if cand == "py" {
            check_cmd.arg("-3.11");
        }
        check_cmd.arg("--version");
        #[cfg(target_os = "windows")]
        check_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = check_cmd.output() {
            if out.status.success() {
                return Ok((PathBuf::from(cand), cli_script));
            }
        }
    }

    Err(
        "See-through sidecar is not installed or configured.\n\
        To install, run 'python vendor/see_through/bootstrap_see_through.py' from PowerShell.".to_string()
    )
}

pub fn segment_character_internal(
    app: Option<&tauri::AppHandle>,
    character_path: &str,
    output_dir_opt: Option<&str>,
) -> Result<CompositionResult, String> {
    let char_p = Path::new(character_path);
    if !char_p.exists() {
        return Err(format!("Character image not found at: {character_path}"));
    }

    // GPU Check
    check_nvidia_gpu_internal()?;

    // Resolve sidecar runner
    let (py_exe, cli_script) = resolve_see_through_python(app)?;

    let out_dir = match output_dir_opt {
        Some(d) => PathBuf::from(d),
        None => {
            let mut base = std::env::temp_dir().join("cia_composition");
            if let Some(app_handle) = app {
                if let Ok(app_dir) = app_handle.path().app_data_dir() {
                    base = app_dir.join("composition");
                }
            }
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create composition dir: {e}"))?;
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(12345);
            base.join(format!("comp_{id}"))
        }
    };

    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let mut cmd = std::process::Command::new(&py_exe);
    if py_exe.to_string_lossy() == "py" {
        cmd.arg("-3.11");
    }
    cmd.arg(&cli_script);
    cmd.args([
        "--input",
        character_path,
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to execute See-through CLI: {e}"))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("See-through layer decomposition failed: {err_msg}"));
    }

    let _stdout_str = String::from_utf8_lossy(&output.stdout);
    let layers_json_path = out_dir.join("layers.json");
    if !layers_json_path.exists() {
        return Err(format!("Expected layers.json not found in output directory: {}", out_dir.display()));
    }

    let layers_content = std::fs::read_to_string(&layers_json_path)
        .map_err(|e| format!("Failed to read layers.json: {e}"))?;

    let mut raw_layers: Vec<LayerItem> = serde_json::from_str(&layers_content)
        .map_err(|e| format!("Failed to parse layers.json: {e}"))?;

    // Populate full paths and base64 thumbnails
    for layer in &mut raw_layers {
        let layer_path = out_dir.join(&layer.file);
        if layer_path.exists() {
            layer.full_path = Some(layer_path.to_string_lossy().to_string());
            if let Ok(bytes) = std::fs::read(&layer_path) {
                let b64 = encode_base64(&bytes);
                layer.thumbnail_base64 = Some(format!("data:image/png;base64,{b64}"));
            }
        }
    }

    Ok(CompositionResult {
        status: "success".to_string(),
        character_path: character_path.to_string(),
        background_path: None,
        output_dir: out_dir.to_string_lossy().to_string(),
        layers_count: raw_layers.len(),
        layers_json_path: layers_json_path.to_string_lossy().to_string(),
        layers: raw_layers,
    })
}

// --- Tauri Commands ---

pub fn gaussian_blur_channel_downscaled(src: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    if radius <= 1.0 || w < 8 || h < 8 {
        return gaussian_blur_channel(src, w, h, radius);
    }
    // Downscale by factor of 4
    let dw = (w / 4).max(2);
    let dh = (h / 4).max(2);
    let mut down = vec![0.0f32; dw * dh];
    let x_scale = w as f32 / dw as f32;
    let y_scale = h as f32 / dh as f32;

    for dy in 0..dh {
        let sy = ((dy as f32 + 0.5) * y_scale) as usize;
        let sy_c = sy.min(h - 1);
        for dx in 0..dw {
            let sx = ((dx as f32 + 0.5) * x_scale) as usize;
            let sx_c = sx.min(w - 1);
            down[dy * dw + dx] = src[sy_c * w + sx_c];
        }
    }

    let down_radius = (radius / 4.0).max(1.5);
    let blurred_down = gaussian_blur_channel(&down, dw, dh, down_radius);

    // Upscale bilinearly back to w x h
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let ty = (y as f32 / y_scale).min((dh - 1) as f32);
        let y0 = ty.floor() as usize;
        let y1 = (y0 + 1).min(dh - 1);
        let wy = ty - y0 as f32;

        let r_out = y * w;
        let r0 = y0 * dw;
        let r1 = y1 * dw;

        for x in 0..w {
            let tx = (x as f32 / x_scale).min((dw - 1) as f32);
            let x0 = tx.floor() as usize;
            let x1 = (x0 + 1).min(dw - 1);
            let wx = tx - x0 as f32;

            let top = blurred_down[r0 + x0] * (1.0 - wx) + blurred_down[r0 + x1] * wx;
            let bot = blurred_down[r1 + x0] * (1.0 - wx) + blurred_down[r1 + x1] * wx;
            out[r_out + x] = top * (1.0 - wy) + bot * wy;
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct PrecomputedCompMasks {
    pub shadow_mask: Option<Vec<f32>>,
    pub edge_mask: Option<Vec<f32>>,
    pub rim_mask: Option<Vec<f32>>,
    pub alpha_channel: Vec<f32>,
}

pub fn precompute_composition_masks(
    char_raw: &RawImage,
    ops: &[CompositionOp],
    w: usize,
    h: usize,
) -> PrecomputedCompMasks {
    let mut alpha_channel = vec![0.0f32; w * h];
    for i in 0..(w * h) {
        alpha_channel[i] = char_raw.data[i * 4 + 3] as f32 / 255.0;
    }

    let mut shadow_mask = None;
    let mut edge_mask = None;
    let mut rim_mask = None;

    for op in ops {
        if !op.enabled {
            continue;
        }
        match op.op_type.as_str() {
            "drop_shadow" => {
                let offset_x = op.params.get("offsetX").and_then(|v| v.as_f64()).unwrap_or(12.0) as f32;
                let offset_y = op.params.get("offsetY").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;
                let blur_radius = op.params.get("blurRadius").and_then(|v| v.as_f64()).unwrap_or(14.0) as f32;

                let mut shifted_alpha = vec![0.0f32; w * h];
                let dx_i = offset_x.round() as i32;
                let dy_i = offset_y.round() as i32;
                for y in 0..h {
                    let sy = y as i32 - dy_i;
                    if sy < 0 || sy >= h as i32 {
                        continue;
                    }
                    for x in 0..w {
                        let sx = x as i32 - dx_i;
                        if sx < 0 || sx >= w as i32 {
                            continue;
                        }
                        shifted_alpha[y * w + x] = alpha_channel[sy as usize * w + sx as usize];
                    }
                }
                shadow_mask = Some(gaussian_blur_channel(&shifted_alpha, w, h, blur_radius));
            }
            "light_wrap" => {
                let edge_width = op.params.get("edgeWidth").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
                edge_mask = Some(extract_inner_edge_mask(&alpha_channel, w, h, edge_width));
            }
            "rim_light" => {
                rim_mask = Some(extract_contour_rim_mask(&alpha_channel, w, h));
            }
            _ => {}
        }
    }

    PrecomputedCompMasks {
        shadow_mask,
        edge_mask,
        rim_mask,
        alpha_channel,
    }
}

pub fn composite_frame_fast(
    bg_rgba: &mut [u8], // W x H x 4
    char_raw: &RawImage, // W x H x 4
    ops: &[CompositionOp],
    masks: &PrecomputedCompMasks,
    w: usize,
    h: usize,
) {
    // 1. Drop shadow
    for op in ops {
        if op.enabled && op.op_type == "drop_shadow" {
            if let Some(ref shadow_mask) = masks.shadow_mask {
                let opacity = op.opacity.clamp(0.0, 1.0);
                for i in 0..(w * h) {
                    let s_val = shadow_mask[i] * opacity;
                    if s_val > 0.001 {
                        let factor = 1.0 - s_val;
                        let idx = i * 4;
                        bg_rgba[idx] = (bg_rgba[idx] as f32 * factor).round().clamp(0.0, 255.0) as u8;
                        bg_rgba[idx + 1] = (bg_rgba[idx + 1] as f32 * factor).round().clamp(0.0, 255.0) as u8;
                        bg_rgba[idx + 2] = (bg_rgba[idx + 2] as f32 * factor).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    // 2. Alpha Over
    for i in 0..(w * h) {
        let idx = i * 4;
        let bg_pix = [bg_rgba[idx], bg_rgba[idx + 1], bg_rgba[idx + 2], bg_rgba[idx + 3]];
        let fg_pix = [char_raw.data[idx], char_raw.data[idx + 1], char_raw.data[idx + 2], char_raw.data[idx + 3]];
        let blended = alpha_over_pixel(bg_pix, fg_pix);
        bg_rgba[idx] = blended[0];
        bg_rgba[idx + 1] = blended[1];
        bg_rgba[idx + 2] = blended[2];
        bg_rgba[idx + 3] = blended[3];
    }

    // 3. Compute background ambient color for tint
    let (mut sum_r, mut sum_g, mut count) = (0.0f64, 0.0f64, 0usize);
    let mut sum_b = 0.0f64;
    for i in (0..(w * h)).step_by(8) {
        if masks.alpha_channel[i] < 0.2 {
            let idx = i * 4;
            sum_r += bg_rgba[idx] as f64;
            sum_g += bg_rgba[idx + 1] as f64;
            sum_b += bg_rgba[idx + 2] as f64;
            count += 1;
        }
    }
    if count == 0 { count = 1; }
    let bg_mean = [
        (sum_r / count as f64) as f32,
        (sum_g / count as f64) as f32,
        (sum_b / count as f64) as f32,
    ];

    // 4. Post-ops
    for op in ops {
        if !op.enabled {
            continue;
        }
        match op.op_type.as_str() {
            "tint" => {
                let opacity = op.opacity.clamp(0.0, 1.0);
                for i in 0..(w * h) {
                    let a = masks.alpha_channel[i];
                    if a > 0.01 {
                        let idx = i * 4;
                        let k = opacity * a;
                        let tr = (bg_rgba[idx] as f32 * (bg_mean[0] / 128.0)).clamp(0.0, 255.0);
                        let tg = (bg_rgba[idx + 1] as f32 * (bg_mean[1] / 128.0)).clamp(0.0, 255.0);
                        let tb = (bg_rgba[idx + 2] as f32 * (bg_mean[2] / 128.0)).clamp(0.0, 255.0);

                        bg_rgba[idx] = (bg_rgba[idx] as f32 * (1.0 - k) + tr * k).round().clamp(0.0, 255.0) as u8;
                        bg_rgba[idx + 1] = (bg_rgba[idx + 1] as f32 * (1.0 - k) + tg * k).round().clamp(0.0, 255.0) as u8;
                        bg_rgba[idx + 2] = (bg_rgba[idx + 2] as f32 * (1.0 - k) + tb * k).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
            "light_wrap" => {
                if let Some(ref edge_mask) = masks.edge_mask {
                    let blur_radius = op.params.get("blurRadius").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32;
                    let opacity = op.opacity.clamp(0.0, 1.0);

                    let mut bg_r = vec![0.0f32; w * h];
                    let mut bg_g = vec![0.0f32; w * h];
                    let mut bg_b = vec![0.0f32; w * h];
                    for i in 0..(w * h) {
                        let idx = i * 4;
                        bg_r[i] = bg_rgba[idx] as f32 / 255.0;
                        bg_g[i] = bg_rgba[idx + 1] as f32 / 255.0;
                        bg_b[i] = bg_rgba[idx + 2] as f32 / 255.0;
                    }
                    let blur_r = gaussian_blur_channel_downscaled(&bg_r, w, h, blur_radius);
                    let blur_g = gaussian_blur_channel_downscaled(&bg_g, w, h, blur_radius);
                    let blur_b = gaussian_blur_channel_downscaled(&bg_b, w, h, blur_radius);

                    for i in 0..(w * h) {
                        let a = masks.alpha_channel[i];
                        let m = edge_mask[i];
                        if a > 0.01 && m > 0.001 {
                            let k = (opacity * m * a).clamp(0.0, 1.0);
                            let idx = i * 4;
                            let cb_r = bg_rgba[idx] as f32 / 255.0;
                            let cb_g = bg_rgba[idx + 1] as f32 / 255.0;
                            let cb_b = bg_rgba[idx + 2] as f32 / 255.0;

                            let cs_r = 1.0 - (1.0 - cb_r) * (1.0 - blur_r[i]);
                            let cs_g = 1.0 - (1.0 - cb_g) * (1.0 - blur_g[i]);
                            let cs_b = 1.0 - (1.0 - cb_b) * (1.0 - blur_b[i]);

                            bg_rgba[idx] = ((cb_r * (1.0 - k) + cs_r * k) * 255.0).round().clamp(0.0, 255.0) as u8;
                            bg_rgba[idx + 1] = ((cb_g * (1.0 - k) + cs_g * k) * 255.0).round().clamp(0.0, 255.0) as u8;
                            bg_rgba[idx + 2] = ((cb_b * (1.0 - k) + cs_b * k) * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
            "rim_light" => {
                if let Some(ref rim_mask) = masks.rim_mask {
                    let opacity = op.opacity.clamp(0.0, 1.0);
                    let rim_color = op.params.get("color")
                        .and_then(|v| v.as_array())
                        .map(|arr| [
                            arr.get(0).and_then(|v| v.as_f64()).unwrap_or(220.0) as f32,
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(240.0) as f32,
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(255.0) as f32,
                        ])
                        .unwrap_or([220.0, 240.0, 255.0]);

                    for i in 0..(w * h) {
                        let a = masks.alpha_channel[i];
                        let m = rim_mask[i];
                        if a > 0.01 && m > 0.001 {
                            let k = opacity * m * a;
                            let idx = i * 4;
                            bg_rgba[idx] = (bg_rgba[idx] as f32 + rim_color[0] * k).clamp(0.0, 255.0).round() as u8;
                            bg_rgba[idx + 1] = (bg_rgba[idx + 1] as f32 + rim_color[1] * k).clamp(0.0, 255.0).round() as u8;
                            bg_rgba[idx + 2] = (bg_rgba[idx + 2] as f32 + rim_color[2] * k).clamp(0.0, 255.0).round() as u8;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn composite_frame_with_ops(
    bg_rgba: &mut [u8], // W x H x 4
    char_raw: &RawImage, // W x H x 4
    ops: &[CompositionOp],
    w: usize,
    h: usize,
) {
    let masks = precompute_composition_masks(char_raw, ops, w, h);
    composite_frame_fast(bg_rgba, char_raw, ops, &masks, w, h);
}

pub fn render_composition_internal(
    app: Option<&tauri::AppHandle>,
    character_path: &str,
    background_path: &str,
    ops_opt: Option<Vec<CompositionOp>>,
    output_dir_opt: Option<&str>,
) -> Result<String, String> {
    use std::io::{Read, Write};
    use tauri::Emitter;

    let char_p = Path::new(character_path);
    let bg_p = Path::new(background_path);

    if !char_p.exists() {
        return Err(format!("Character image not found at: {character_path}"));
    }
    if !bg_p.exists() {
        return Err(format!("Background file not found at: {background_path}"));
    }

    let ffmpeg_bin_opt = app.and_then(|a| crate::probe::get_ffmpeg_binary(a).ok());

    // 1. Validate Character PNG (strictly checks alpha channel)
    let char_raw = validate_and_load_character_png(char_p, ffmpeg_bin_opt.as_deref())?;

    let ops = ops_opt.unwrap_or_else(get_default_composition_ops);

    let base_out = match output_dir_opt {
        Some(d) => PathBuf::from(d),
        None => {
            let mut base = std::env::temp_dir().join("cia_composition");
            if let Some(app_handle) = app {
                if let Ok(app_dir) = app_handle.path().app_data_dir() {
                    base = app_dir.join("composition_renders");
                }
            }
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create render dir: {e}"))?;
            base
        }
    };
    std::fs::create_dir_all(&base_out).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(123456);

    let bg_ext = bg_p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_video = ["mp4", "mkv", "webm", "mov", "avi"].contains(&bg_ext.as_str());

    if !is_video {
        // STATIC IMAGE COMPOSITION
        let bg_raw = load_image_rgba(bg_p, ffmpeg_bin_opt.as_deref())?;
        let (w, h) = (bg_raw.width, bg_raw.height);

        // Scale character to fit canvas
        let char_scaled = resize_bilinear_rgba(&char_raw, w, h);
        let mut canvas_data = bg_raw.data.clone();

        composite_frame_with_ops(&mut canvas_data, &char_scaled, &ops, w, h);

        let out_path = base_out.join(format!("composition_{timestamp}.png"));
        let out_img = RawImage {
            width: w,
            height: h,
            data: canvas_data,
        };

        save_image_rgba(&out_img, &out_path, ffmpeg_bin_opt.as_deref())?;

        if let Some(app_handle) = app {
            let _ = app_handle.emit("comp-progress", CompositionProgress {
                phase: "DONE".to_string(),
                percent: 100,
                current_frame: 1,
                total_frames: 1,
                message: "Image composition complete".to_string(),
            });
        }

        Ok(out_path.to_string_lossy().to_string())
    } else {
        // VIDEO BACKGROUND COMPOSITION
        let ffmpeg_bin = ffmpeg_bin_opt.unwrap_or_else(|| PathBuf::from("ffmpeg"));

        let probe = crate::probe::probe_media_internal(background_path, None)
            .map_err(|e| format!("Background video could not be read: {e}"))?;

        let fps = if probe.fps > 0.0 { probe.fps } else { 30.0 };
        let total_frames = ((probe.duration * fps).ceil() as u32).max(1);
        let (w, h) = (probe.width.max(2) as usize, probe.height.max(2) as usize);

        // Scale character to match video canvas
        let char_scaled = resize_bilinear_rgba(&char_raw, w, h);
        let precomputed_masks = precompute_composition_masks(&char_scaled, &ops, w, h);

        let out_path = base_out.join(format!("composition_{timestamp}.mp4"));
        let cache_file = base_out.join(format!("comp_raw_{timestamp}.raw"));

        // 1. Decode background frames to raw cache file
        let mut decode_cmd = std::process::Command::new(&ffmpeg_bin);
        decode_cmd.args([
            "-y",
            "-i", background_path,
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-an",
            cache_file.to_str().unwrap(),
        ]);
        decode_cmd.stdout(std::process::Stdio::null());
        decode_cmd.stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        decode_cmd.creation_flags(CREATE_NO_WINDOW);

        let mut decode_child = decode_cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg decoder: {e}"))?;

        let decode_status = decode_child.wait()
            .map_err(|e| format!("Decoder wait failed: {e}"))?;

        if !decode_status.success() {
            let _ = std::fs::remove_file(&cache_file);
            return Err("FFmpeg video decoding failed".to_string());
        }

        // 2. Launch ffmpeg encoder process
        let mut encode_cmd = std::process::Command::new(&ffmpeg_bin);
        encode_cmd.args([
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &format!("{w}x{h}"),
            "-r", &format!("{fps}"),
            "-i", "-",
            "-i", background_path,
            "-map", "0:v:0",
            "-map", "1:a:0?",
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
            "-shortest",
            out_path.to_str().unwrap(),
        ]);
        encode_cmd.stdin(std::process::Stdio::piped());
        encode_cmd.stdout(std::process::Stdio::null());
        encode_cmd.stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        encode_cmd.creation_flags(CREATE_NO_WINDOW);

        let mut encode_child = encode_cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg encoder: {e}"))?;

        let mut encode_stdin = encode_child.stdin.take()
            .ok_or_else(|| "Failed to open ffmpeg encode stdin".to_string())?;

        let frame_bytes = w * h * 4;
        let mut frame_buf = vec![0u8; frame_bytes];
        let mut raw_file = std::io::BufReader::new(
            std::fs::File::open(&cache_file).map_err(|e| format!("Failed to open cache file: {e}"))?
        );
        let mut current_frame = 0u32;

        while raw_file.read_exact(&mut frame_buf).is_ok() {
            current_frame += 1;

            composite_frame_fast(&mut frame_buf, &char_scaled, &ops, &precomputed_masks, w, h);

            if encode_stdin.write_all(&frame_buf).is_err() {
                break;
            }

            if current_frame % 10 == 0 || current_frame == total_frames {
                let pct = ((current_frame as f64 / total_frames as f64) * 100.0).clamp(0.0, 100.0) as u32;
                if let Some(app_handle) = app {
                    let _ = app_handle.emit("comp-progress", CompositionProgress {
                        phase: "COMPOSITING".to_string(),
                        percent: pct,
                        current_frame,
                        total_frames,
                        message: format!("Compositing frame {current_frame}/{total_frames}"),
                    });
                }
            }
        }

        drop(encode_stdin);
        let encode_status = encode_child.wait()
            .map_err(|e| format!("Encoder wait failed: {e}"))?;

        let _ = std::fs::remove_file(&cache_file);

        if !encode_status.success() {
            return Err("FFmpeg video encoding failed".to_string());
        }

        if let Some(app_handle) = app {
            let _ = app_handle.emit("comp-progress", CompositionProgress {
                phase: "DONE".to_string(),
                percent: 100,
                current_frame: total_frames,
                total_frames,
                message: "Video composition complete".to_string(),
            });
        }

        Ok(out_path.to_string_lossy().to_string())
    }
}

// --- Tauri Commands ---

#[tauri::command]
pub fn check_gpu_status() -> Result<String, String> {
    check_nvidia_gpu_internal()
}

#[tauri::command]
pub fn segment_character(
    app: tauri::AppHandle,
    character_path: String,
) -> Result<CompositionResult, String> {
    segment_character_internal(Some(&app), &character_path, None)
}

#[tauri::command]
pub fn save_composition_project(
    project: CompProject,
    target_path: Option<String>,
) -> Result<String, String> {
    let path_str = match target_path {
        Some(p) => p,
        None => {
            let base = std::env::temp_dir().join("cia_composition");
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create dir: {e}"))?;
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(12345);
            base.join(format!("comp_project_{id}.json")).to_string_lossy().to_string()
        }
    };

    let json_content = serde_json::to_string_pretty(&project)
        .map_err(|e| format!("Failed to serialize comp project: {e}"))?;

    std::fs::write(&path_str, json_content)
        .map_err(|e| format!("Failed to write comp project to {path_str}: {e}"))?;

    Ok(path_str)
}

#[tauri::command]
pub fn render_composition(
    app: tauri::AppHandle,
    character_path: String,
    background_path: String,
    ops: Option<Vec<CompositionOp>>,
) -> Result<String, String> {
    render_composition_internal(Some(&app), &character_path, &background_path, ops, None)
}

#[tauri::command]
pub fn get_default_composition_ops_cmd() -> Vec<CompositionOp> {
    get_default_composition_ops()
}
