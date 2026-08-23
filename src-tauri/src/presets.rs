use std::fs;
use std::path::{Path, PathBuf};
use crate::audio::AudioMixConfig;
use crate::composition::CompProject;
use crate::plan::ProjectPlan;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ShakeDirection {
    X,
    Y,
    Radial,
    Random,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PunchEasing {
    Linear,
    EaseOut,
    Bounce,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ZoomDriftDirection {
    In,
    Out,
    Alternate,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TransitionTypeChoice {
    HardCut,
    CrossDissolve,
    Wipe,
    ZoomThrough,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SpeedRampStyle {
    Linear,
    Bezier,
    Exponential,
}

fn default_true() -> bool { true }
fn default_shake_intensity() -> f32 { 0.5 }
fn default_shake_freq_hz() -> f32 { 15.0 }
fn default_shake_decay_ms() -> f32 { 200.0 }
fn default_shake_direction() -> ShakeDirection { ShakeDirection::Radial }
fn default_shake_roll_deg() -> f32 { 5.0 }
fn default_shake_noise_seed() -> u32 { 1337 }

fn default_punch_in_scale() -> f32 { 1.15 }
fn default_punch_in_duration_ms() -> f32 { 150.0 }
fn default_punch_in_easing() -> PunchEasing { PunchEasing::EaseOut }
fn default_zoom_drift_speed() -> f32 { 0.005 }
fn default_zoom_drift_direction() -> ZoomDriftDirection { ZoomDriftDirection::In }

fn default_rgb_split_intensity() -> f32 { 0.3 }
fn default_flash_intensity() -> f32 { 0.5 }
fn default_glow_threshold() -> f32 { 0.8 }
fn default_glow_radius_px() -> f32 { 6.0 }
fn default_vignette_strength_f32() -> f32 { 0.3 }
fn default_grain_amount() -> f32 { 0.1 }
fn default_color_shift_hue_deg() -> f32 { 0.0 }

fn default_transition_type_choice() -> TransitionTypeChoice { TransitionTypeChoice::HardCut }
fn default_transition_duration_ms() -> f32 { 80.0 }
fn default_reverse_cut_probability() -> f32 { 0.3 }
fn default_freeze_frame_probability() -> f32 { 0.1 }
fn default_speed_ramp_style() -> SpeedRampStyle { SpeedRampStyle::Exponential }
fn default_ramp_acceleration() -> f32 { 1.5 }
fn default_ramp_deceleration() -> f32 { 1.5 }

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemapParams {
    // SHAKES (7)
    #[serde(default = "default_shake_intensity")]
    pub shake_intensity: f32, // 0.0 .. 1.0
    #[serde(default = "default_shake_freq_hz")]
    pub shake_freq_hz: f32, // 1.0 .. 30.0
    #[serde(default = "default_shake_decay_ms")]
    pub shake_decay_ms: f32, // 50.0 .. 500.0
    #[serde(default = "default_shake_direction")]
    pub shake_direction: ShakeDirection,
    #[serde(default = "default_true")]
    pub shake_on_beats_only: bool,
    #[serde(default = "default_shake_roll_deg")]
    pub shake_roll_deg: f32, // 0.0 .. 15.0
    #[serde(default = "default_shake_noise_seed")]
    pub shake_noise_seed: u32,

    // ZOOM (7)
    #[serde(default = "default_punch_in_scale")]
    pub punch_in_scale: f32, // 1.0 .. 1.5
    #[serde(default = "default_punch_in_duration_ms")]
    pub punch_in_duration_ms: f32, // 50.0 .. 400.0
    #[serde(default = "default_punch_in_easing")]
    pub punch_in_easing: PunchEasing,
    #[serde(default = "default_zoom_drift_speed")]
    pub zoom_drift_speed: f32, // 0.0 .. 0.02
    #[serde(default = "default_zoom_drift_direction")]
    pub zoom_drift_direction: ZoomDriftDirection,
    #[serde(default = "default_true")]
    pub punch_on_downbeats_only: bool,
    #[serde(default = "default_true")]
    pub zoom_reset_between_cuts: bool,

    // AMBIANCE (7)
    #[serde(default = "default_rgb_split_intensity")]
    pub rgb_split_intensity: f32, // 0.0 .. 1.0
    #[serde(default = "default_flash_intensity")]
    pub flash_intensity: f32, // 0.0 .. 1.0
    #[serde(default = "default_glow_threshold")]
    pub glow_threshold: f32, // 0.0 .. 1.0
    #[serde(default = "default_glow_radius_px")]
    pub glow_radius_px: f32, // 0.0 .. 20.0
    #[serde(default = "default_vignette_strength_f32")]
    pub vignette_strength: f32, // 0.0 .. 1.0
    #[serde(default = "default_grain_amount")]
    pub grain_amount: f32, // 0.0 .. 0.5
    #[serde(default = "default_color_shift_hue_deg")]
    pub color_shift_hue_deg: f32, // 0.0 .. 360.0

    // TRANSITIONS (7)
    #[serde(default = "default_transition_type_choice")]
    pub transition_type: TransitionTypeChoice,
    #[serde(default = "default_transition_duration_ms")]
    pub transition_duration_ms: f32, // 0.0 .. 200.0
    #[serde(default = "default_reverse_cut_probability")]
    pub reverse_cut_probability: f32, // 0.0 .. 1.0
    #[serde(default = "default_freeze_frame_probability")]
    pub freeze_frame_probability: f32, // 0.0 .. 1.0
    #[serde(default = "default_speed_ramp_style")]
    pub speed_ramp_style: SpeedRampStyle,
    #[serde(default = "default_ramp_acceleration")]
    pub ramp_acceleration: f32, // 0.5 .. 4.0
    #[serde(default = "default_ramp_deceleration")]
    pub ramp_deceleration: f32, // 0.5 .. 4.0
}

impl Default for RemapParams {
    fn default() -> Self {
        get_preset_params("AGGRESSIVE_JUGG")
    }
}

pub fn get_preset_params(name: &str) -> RemapParams {
    let name_up = name.to_uppercase();
    match name_up.as_str() {
        "LIQUID_FLOW" | "FLOW" | "SMOOTH" => RemapParams {
            shake_intensity: 0.25,
            shake_freq_hz: 8.0,
            shake_decay_ms: 350.0,
            shake_direction: ShakeDirection::X,
            shake_on_beats_only: false,
            shake_roll_deg: 2.0,
            shake_noise_seed: 2024,

            punch_in_scale: 1.08,
            punch_in_duration_ms: 280.0,
            punch_in_easing: PunchEasing::EaseOut,
            zoom_drift_speed: 0.003,
            zoom_drift_direction: ZoomDriftDirection::Out,
            punch_on_downbeats_only: false,
            zoom_reset_between_cuts: false,

            rgb_split_intensity: 0.15,
            flash_intensity: 0.1,
            glow_threshold: 0.85,
            glow_radius_px: 4.0,
            vignette_strength: 0.2,
            grain_amount: 0.05,
            color_shift_hue_deg: 15.0,

            transition_type: TransitionTypeChoice::CrossDissolve,
            transition_duration_ms: 150.0,
            reverse_cut_probability: 0.0,
            freeze_frame_probability: 0.0,
            speed_ramp_style: SpeedRampStyle::Bezier,
            ramp_acceleration: 1.1,
            ramp_deceleration: 1.1,
        },
        "GROOVE_VIBE" | "VIBE" | "HYBRID" => RemapParams {
            shake_intensity: 0.55,
            shake_freq_hz: 12.0,
            shake_decay_ms: 220.0,
            shake_direction: ShakeDirection::Random,
            shake_on_beats_only: true,
            shake_roll_deg: 4.0,
            shake_noise_seed: 4242,

            punch_in_scale: 1.18,
            punch_in_duration_ms: 180.0,
            punch_in_easing: PunchEasing::EaseOut,
            zoom_drift_speed: 0.006,
            zoom_drift_direction: ZoomDriftDirection::Alternate,
            punch_on_downbeats_only: true,
            zoom_reset_between_cuts: true,

            rgb_split_intensity: 0.4,
            flash_intensity: 0.4,
            glow_threshold: 0.75,
            glow_radius_px: 6.0,
            vignette_strength: 0.35,
            grain_amount: 0.12,
            color_shift_hue_deg: 0.0,

            transition_type: TransitionTypeChoice::ZoomThrough,
            transition_duration_ms: 90.0,
            reverse_cut_probability: 0.2,
            freeze_frame_probability: 0.05,
            speed_ramp_style: SpeedRampStyle::Linear,
            ramp_acceleration: 1.6,
            ramp_deceleration: 1.6,
        },
        _ => RemapParams {
            // AGGRESSIVE_JUGG / HARD
            shake_intensity: 0.9,
            shake_freq_hz: 22.0,
            shake_decay_ms: 120.0,
            shake_direction: ShakeDirection::Radial,
            shake_on_beats_only: true,
            shake_roll_deg: 8.0,
            shake_noise_seed: 1337,

            punch_in_scale: 1.35,
            punch_in_duration_ms: 100.0,
            punch_in_easing: PunchEasing::Bounce,
            zoom_drift_speed: 0.01,
            zoom_drift_direction: ZoomDriftDirection::In,
            punch_on_downbeats_only: true,
            zoom_reset_between_cuts: true,

            rgb_split_intensity: 0.8,
            flash_intensity: 0.9,
            glow_threshold: 0.6,
            glow_radius_px: 10.0,
            vignette_strength: 0.5,
            grain_amount: 0.2,
            color_shift_hue_deg: 0.0,

            transition_type: TransitionTypeChoice::HardCut,
            transition_duration_ms: 50.0,
            reverse_cut_probability: 0.45,
            freeze_frame_probability: 0.15,
            speed_ramp_style: SpeedRampStyle::Exponential,
            ramp_acceleration: 2.5,
            ramp_deceleration: 2.5,
        },
    }
}

pub fn get_presets_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let p = PathBuf::from(appdata).join("cia_app").join("presets");
        let _ = fs::create_dir_all(&p);
        p
    } else {
        let p = PathBuf::from("presets");
        let _ = fs::create_dir_all(&p);
        p
    }
}

pub fn save_preset_internal(name: &str, params: &RemapParams) -> Result<String, String> {
    let dir = get_presets_dir();
    let sanitized_name = name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
    let file_path = dir.join(format!("{sanitized_name}.json"));
    let json_str = serde_json::to_string_pretty(params)
        .map_err(|e| format!("Failed to serialize preset: {e}"))?;
    fs::write(&file_path, json_str)
        .map_err(|e| format!("Failed to write preset to {}: {e}", file_path.display()))?;
    Ok(file_path.to_string_lossy().to_string())
}

pub fn load_preset_internal(name: &str) -> Result<RemapParams, String> {
    let name_up = name.to_uppercase();
    if name_up == "AGGRESSIVE_JUGG" || name_up == "LIQUID_FLOW" || name_up == "GROOVE_VIBE" {
        return Ok(get_preset_params(&name_up));
    }

    let dir = get_presets_dir();
    let file_path = dir.join(format!("{name}.json"));
    if file_path.exists() {
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read preset at {}: {e}", file_path.display()))?;
        let parsed: RemapParams = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse preset JSON: {e}"))?;
        Ok(parsed)
    } else {
        // Fallback check case-insensitive match
        Ok(get_preset_params(name))
    }
}

pub fn list_presets_internal() -> Result<Vec<String>, String> {
    let mut presets = vec![
        "AGGRESSIVE_JUGG".to_string(),
        "LIQUID_FLOW".to_string(),
        "GROOVE_VIBE".to_string(),
    ];

    let dir = get_presets_dir();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !presets.iter().any(|p| p.eq_ignore_ascii_case(stem)) {
                        presets.push(stem.to_string());
                    }
                }
            }
        }
    }

    Ok(presets)
}

// ─── T38 Project State Persistence ──────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub schema_version: u32,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub scene_path: Option<String>,
    #[serde(default)]
    pub drums_path: Option<String>,
    #[serde(default)]
    pub audio_path: Option<String>,
    #[serde(default)]
    pub character_path: Option<String>,
    #[serde(default)]
    pub background_path: Option<String>,
    #[serde(default)]
    pub plan: Option<ProjectPlan>,
    #[serde(default)]
    pub composition_project: Option<CompProject>,
    #[serde(default)]
    pub audio_mix: Option<AudioMixConfig>,
    #[serde(default)]
    pub remap_params: Option<RemapParams>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            project_name: Some("Untitled Jugg Project".to_string()),
            scene_path: None,
            drums_path: None,
            audio_path: None,
            character_path: None,
            background_path: None,
            plan: None,
            composition_project: None,
            audio_mix: Some(AudioMixConfig::default()),
            remap_params: Some(RemapParams::default()),
        }
    }
}

pub fn save_project_state_internal(path: &str, state: &ProjectState) -> Result<String, String> {
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json_str = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize project state: {e}"))?;
    fs::write(p, json_str)
        .map_err(|e| format!("Failed to write project state to {path}: {e}"))?;
    Ok(path.to_string())
}

pub fn load_project_state_internal(path: &str) -> Result<ProjectState, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read project state from {path}: {e}"))?;
    let state: ProjectState = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse project state JSON from {path}: {e}"))?;
    Ok(state)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn save_preset(name: String, params: RemapParams) -> Result<String, String> {
    save_preset_internal(&name, &params)
}

#[tauri::command]
pub fn load_preset(name: String) -> Result<RemapParams, String> {
    load_preset_internal(&name)
}

#[tauri::command]
pub fn list_presets() -> Result<Vec<String>, String> {
    list_presets_internal()
}

#[tauri::command]
pub fn save_project_state(path: String, state: ProjectState) -> Result<String, String> {
    save_project_state_internal(&path, &state)
}

#[tauri::command]
pub fn load_project_state(path: String) -> Result<ProjectState, String> {
    load_project_state_internal(&path)
}
