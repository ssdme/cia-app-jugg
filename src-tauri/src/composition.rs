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
