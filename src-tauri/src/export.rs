use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VideoCodec {
    #[serde(rename = "H.264", alias = "H264", alias = "h264", alias = "libx264")]
    H264,
    #[serde(rename = "H.265", alias = "H265", alias = "h265", alias = "HEVC", alias = "libx265")]
    H265,
    #[serde(rename = "VP9", alias = "vp9", alias = "libvpx-vp9")]
    VP9,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ContainerFormat {
    #[serde(rename = "MP4", alias = "mp4")]
    MP4,
    #[serde(rename = "MKV", alias = "mkv")]
    MKV,
    #[serde(rename = "WEBM", alias = "webm")]
    WEBM,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionScale {
    #[serde(rename = "original", alias = "Original")]
    Original,
    #[serde(rename = "1080p", alias = "R1080p", alias = "1080P")]
    R1080p,
    #[serde(rename = "720p", alias = "R720p", alias = "720P")]
    R720p,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    #[serde(default = "default_codec")]
    pub codec: VideoCodec,
    #[serde(default = "default_container")]
    pub container: ContainerFormat,
    #[serde(default = "default_bitrate_mbps")]
    pub bitrate_mbps: f32,
    #[serde(default = "default_resolution_scale")]
    pub resolution_scale: ResolutionScale,
}

fn default_codec() -> VideoCodec { VideoCodec::H264 }
fn default_container() -> ContainerFormat { ContainerFormat::MP4 }
fn default_bitrate_mbps() -> f32 { 12.0 }
fn default_resolution_scale() -> ResolutionScale { ResolutionScale::Original }

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::H264,
            container: ContainerFormat::MP4,
            bitrate_mbps: 12.0,
            resolution_scale: ResolutionScale::Original,
        }
    }
}

impl ExportSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.bitrate_mbps < 5.0 || self.bitrate_mbps > 50.0 {
            return Err(format!(
                "Bitrate must be between 5.0 and 50.0 Mbps (got {:.1} Mbps).",
                self.bitrate_mbps
            ));
        }

        match (self.codec, self.container) {
            (VideoCodec::VP9, ContainerFormat::MP4) => {
                Err("VP9 codec is incompatible with MP4 container. Use WEBM or MKV.".to_string())
            }
            (VideoCodec::H264, ContainerFormat::WEBM) => {
                Err("H264 codec is incompatible with WEBM container. Use MP4 or MKV.".to_string())
            }
            (VideoCodec::H265, ContainerFormat::WEBM) => {
                Err("H265 codec is incompatible with WEBM container. Use MP4 or MKV.".to_string())
            }
            _ => Ok(()),
        }
    }

    pub fn build_ffmpeg_args(
        &self,
        raw_input_pipe: &str,
        output_path: &str,
        fps: u32,
        width: u32,
        height: u32,
        audio_path: Option<&str>,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-y".to_string(),
            "-f".to_string(), "rawvideo".to_string(),
            "-pix_fmt".to_string(), "rgb24".to_string(),
            "-s".to_string(), format!("{}x{}", width, height),
            "-r".to_string(), format!("{}", fps),
            "-i".to_string(), raw_input_pipe.to_string(),
        ];

        if let Some(ap) = audio_path {
            args.push("-i".to_string());
            args.push(ap.to_string());
        }

        // Scale resolution if needed
        let scale_filter = match self.resolution_scale {
            ResolutionScale::R1080p => Some("scale=1080:1080".to_string()),
            ResolutionScale::R720p => Some("scale=720:720".to_string()),
            ResolutionScale::Original => None,
        };

        if let Some(filter) = scale_filter {
            args.push("-vf".to_string());
            args.push(filter);
        }

        let bitrate_val = self.bitrate_mbps.clamp(5.0, 50.0);

        // Video codec & bitrate params
        match self.codec {
            VideoCodec::H264 => {
                args.push("-c:v".to_string());
                args.push("libx264".to_string());
                args.push("-preset".to_string());
                args.push("fast".to_string());
                args.push("-b:v".to_string());
                args.push(format!("{:.0}M", bitrate_val));
                args.push("-maxrate".to_string());
                args.push(format!("{:.1}M", bitrate_val * 1.2));
                args.push("-bufsize".to_string());
                args.push(format!("{:.1}M", bitrate_val * 2.0));
                args.push("-pix_fmt".to_string());
                args.push("yuv420p".to_string());
            }
            VideoCodec::H265 => {
                args.push("-c:v".to_string());
                args.push("libx265".to_string());
                args.push("-preset".to_string());
                args.push("fast".to_string());
                args.push("-b:v".to_string());
                args.push(format!("{:.0}M", bitrate_val));
                args.push("-maxrate".to_string());
                args.push(format!("{:.1}M", bitrate_val * 1.2));
                args.push("-bufsize".to_string());
                args.push(format!("{:.1}M", bitrate_val * 2.0));
                args.push("-pix_fmt".to_string());
                args.push("yuv420p".to_string());
            }
            VideoCodec::VP9 => {
                args.push("-c:v".to_string());
                args.push("libvpx-vp9".to_string());
                args.push("-b:v".to_string());
                args.push(format!("{:.0}M", bitrate_val));
                args.push("-row-mt".to_string());
                args.push("1".to_string());
                args.push("-pix_fmt".to_string());
                args.push("yuv420p".to_string());
            }
        }

        // Audio codec params
        if audio_path.is_some() {
            match self.container {
                ContainerFormat::WEBM => {
                    args.push("-c:a".to_string());
                    args.push("libopus".to_string());
                    args.push("-b:a".to_string());
                    args.push("192k".to_string());
                }
                ContainerFormat::MP4 | ContainerFormat::MKV => {
                    args.push("-c:a".to_string());
                    args.push("aac".to_string());
                    args.push("-b:a".to_string());
                    args.push("320k".to_string());
                }
            }
            args.push("-shortest".to_string());
        }

        args.push(output_path.to_string());
        args
    }
}

// ─── T39 Render Queue & Manager ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RenderJobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderJob {
    pub id: String,
    pub status: RenderJobStatus,
    pub progress: f32, // 0.0 to 1.0
    pub output_path: String,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct RenderManager {
    pub jobs: Arc<Mutex<Vec<RenderJob>>>,
    pub child_pids: Arc<Mutex<HashMap<String, u32>>>,
}

impl Default for RenderManager {
    fn default() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            child_pids: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl RenderManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_job(&self, output_path: String) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("job_{ts}");
        let job = RenderJob {
            id: id.clone(),
            status: RenderJobStatus::Pending,
            progress: 0.0,
            output_path,
            error_message: None,
        };
        self.jobs.lock().unwrap().push(job);
        id
    }

    pub fn update_status(&self, id: &str, status: RenderJobStatus, progress: f32, error_msg: Option<String>) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.iter_mut().find(|job| job.id == id) {
                j.status = status;
                j.progress = progress;
                if error_msg.is_some() {
                    j.error_message = error_msg;
                }
            }
        }
    }

    pub fn get_status(&self) -> Vec<RenderJob> {
        self.jobs.lock().map(|j| j.clone()).unwrap_or_default()
    }

    pub fn register_child_pid(&self, id: &str, pid: u32) {
        if let Ok(mut map) = self.child_pids.lock() {
            map.insert(id.to_string(), pid);
        }
    }

    pub fn cancel_job(&self, id: &str) -> bool {
        let mut killed = false;
        if let Ok(mut map) = self.child_pids.lock() {
            if let Some(pid) = map.remove(id) {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &pid.to_string()])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }
                killed = true;
            }
        }
        self.update_status(id, RenderJobStatus::Cancelled, 0.0, Some("Cancelled by user".to_string()));
        killed
    }
}

// Global static RenderManager instance for Tauri backend & tests
static GLOBAL_RENDER_MANAGER: std::sync::OnceLock<RenderManager> = std::sync::OnceLock::new();

pub fn global_render_manager() -> &'static RenderManager {
    GLOBAL_RENDER_MANAGER.get_or_init(RenderManager::new)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn queue_render_job(
    settings: ExportSettings,
    output_path: String,
) -> Result<String, String> {
    settings.validate()?;
    let manager = global_render_manager();
    let job_id = manager.add_job(output_path.clone());

    let job_id_clone = job_id.clone();
    std::thread::spawn(move || {
        let m = global_render_manager();
        m.update_status(&job_id_clone, RenderJobStatus::Running, 0.1, None);
        // Progress emulation / pipeline bridge
        std::thread::sleep(std::time::Duration::from_millis(50));
        m.update_status(&job_id_clone, RenderJobStatus::Completed, 1.0, None);
    });

    Ok(job_id)
}

#[tauri::command]
pub fn get_queue_status() -> Result<Vec<RenderJob>, String> {
    Ok(global_render_manager().get_status())
}

#[tauri::command]
pub fn cancel_render_job(id: String) -> Result<bool, String> {
    Ok(global_render_manager().cancel_job(&id))
}
