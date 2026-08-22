use tauri::Manager;
use crate::beat::get_binary_path;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub audio_channels: u32,
    pub audio_sample_rate: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct CropInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub out_w: u32,
    pub out_h: u32,
}

#[inline(always)]
pub fn mirror_coordinate(coord: i64, max_dim: usize) -> usize {
    if max_dim <= 1 {
        return 0;
    }
    let dim = max_dim as i64;
    let mut c = coord;
    if c < 0 {
        c = -c;
    }
    if c >= dim {
        let diff = c - dim;
        c = dim - 1 - diff;
    }
    c.clamp(0, dim - 1) as usize
}

#[inline(always)]
pub fn sample_pixel_mirrored(frame_data: &[u8], width: usize, height: usize, x: i64, y: i64) -> [u8; 3] {
    let w_i = width as i64;
    let h_i = height as i64;
    let (mx, my) = if x >= 0 && x < w_i && y >= 0 && y < h_i {
        (x as usize, y as usize)
    } else {
        (mirror_coordinate(x, width), mirror_coordinate(y, height))
    };
    let idx = (my * width + mx) * 3;
    if idx + 2 < frame_data.len() {
        [frame_data[idx], frame_data[idx + 1], frame_data[idx + 2]]
    } else {
        [0, 0, 0]
    }
}

#[inline(always)]
pub fn sample_bilinear_mirrored(frame_data: &[u8], width: usize, height: usize, fx: f64, fy: f64) -> [u8; 3] {
    let x0 = fx.floor() as i64;
    let y0 = fy.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let wx = fx - (x0 as f64);
    let wy = fy - (y0 as f64);

    let p00 = sample_pixel_mirrored(frame_data, width, height, x0, y0);
    let p10 = sample_pixel_mirrored(frame_data, width, height, x1, y0);
    let p01 = sample_pixel_mirrored(frame_data, width, height, x0, y1);
    let p11 = sample_pixel_mirrored(frame_data, width, height, x1, y1);

    let mut out = [0u8; 3];
    for c in 0..3 {
        let top = (p00[c] as f64) * (1.0 - wx) + (p10[c] as f64) * wx;
        let bottom = (p01[c] as f64) * (1.0 - wx) + (p11[c] as f64) * wx;
        let val = top * (1.0 - wy) + bottom * wy;
        out[c] = val.round().clamp(0.0, 255.0) as u8;
    }
    out
}

pub fn compute_borderless_scale(src_w: u32, src_h: u32, target_w: u32, target_h: u32) -> (f64, f64) {
    if src_w == 0 || src_h == 0 {
        return (1.0, 1.0);
    }
    let scale_x = (target_w as f64) / (src_w as f64);
    let scale_y = (target_h as f64) / (src_h as f64);
    (scale_x, scale_y)
}

pub fn compute_crop_to_fill(src_w: u32, src_h: u32, aspect_w: u32, aspect_h: u32) -> CropInfo {
    let aspect_w = if aspect_w == 0 { 1080 } else { aspect_w };
    let aspect_h = if aspect_h == 0 { 1080 } else { aspect_h };

    let (out_w, out_h) = if aspect_w >= 100 && aspect_h >= 100 {
        (aspect_w & !1, aspect_h & !1)
    } else if aspect_w >= aspect_h {
        let target_ar = (aspect_w as f64) / (aspect_h as f64);
        let ow = ((1080.0 * target_ar).round() as u32) & !1;
        let oh = 1080u32;
        (ow.max(2), oh)
    } else {
        let target_ar = (aspect_h as f64) / (aspect_w as f64);
        let ow = 1080u32;
        let oh = ((1080.0 * target_ar).round() as u32) & !1;
        (ow, oh.max(2))
    };

    // Borderless: always stretch full source to target dimensions without any cropping
    CropInfo {
        x: 0,
        y: 0,
        width: src_w,
        height: src_h,
        out_w,
        out_h,
    }
}

pub fn get_ffmpeg_binary(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // 1. Check in app_data_dir/binaries/ffmpeg.exe
    if let Ok(data_dir) = app.path().app_data_dir() {
        let in_data = data_dir.join("binaries").join("ffmpeg.exe");
        if in_data.exists() {
            return Ok(in_data);
        }
        let in_data_root = data_dir.join("ffmpeg.exe");
        if in_data_root.exists() {
            return Ok(in_data_root);
        }
    }

    // 2. Check local binaries dir
    let direct = get_binary_path(app, "ffmpeg.exe");
    if direct.exists() {
        return Ok(direct);
    }

    // 3. Check system PATH
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("ffmpeg")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = std::path::PathBuf::from(first_line.trim());
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }
    }

    // 4. Download and extract ffmpeg-release-essentials
    download_and_extract_ffmpeg(app)
}

pub fn get_ffprobe_binary(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // Derive ffprobe path from ffmpeg path (they ship together)
    let ffmpeg_path = get_ffmpeg_binary(app)?;
    let ffprobe_path = ffmpeg_path.with_file_name("ffprobe.exe");
    if ffprobe_path.exists() {
        return Ok(ffprobe_path);
    }
    // Fallback: check system PATH
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("ffprobe")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = std::path::PathBuf::from(first_line.trim());
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }
    }
    Err("ffprobe binary not found (required for fallback probe)".to_string())
}

pub fn download_and_extract_ffmpeg(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
    let bin_dir = data_dir.join("binaries");
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", bin_dir.display()))?;

    let ffmpeg_dest = bin_dir.join("ffmpeg.exe");
    if ffmpeg_dest.exists() {
        return Ok(ffmpeg_dest);
    }

    // Check temp extracted folder if already downloaded
    let temp_extracted = std::env::temp_dir().join("ffmpeg-essentials");
    if temp_extracted.exists() {
        if let Ok(entries) = std::fs::read_dir(&temp_extracted) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin").join("ffmpeg.exe");
                if bin.exists() {
                    let _ = std::fs::copy(&bin, &ffmpeg_dest);
                    if ffmpeg_dest.exists() {
                        return Ok(ffmpeg_dest);
                    }
                }
            }
        }
    }

    // Fallback: download essentials zip from gyan.dev
    let zip_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    let zip_dest = data_dir.join("ffmpeg-download.zip");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '{}' -OutFile '{}'; Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            zip_url,
            zip_dest.display(),
            zip_dest.display(),
            bin_dir.display()
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to download ffmpeg: {e}"))?;

        if !status.success() {
            return Err("Failed to download or extract FFmpeg".to_string());
        }

        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let p = entry.path().join("bin").join("ffmpeg.exe");
                if p.exists() {
                    let _ = std::fs::copy(&p, &ffmpeg_dest);
                    let _ = std::fs::remove_file(&zip_dest);
                    return Ok(ffmpeg_dest);
                }
            }
        }
    }

    if ffmpeg_dest.exists() {
        Ok(ffmpeg_dest)
    } else {
        Err("FFmpeg binary could not be located".to_string())
    }
}

#[tauri::command]
pub fn pick_file(kind: String) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    match kind.as_str() {
        "video" => {
            dialog = dialog.add_filter(
                "Video Files (*.mp4, *.mkv, *.webm, *.mov, *.avi)",
                &["mp4", "mkv", "webm", "mov", "avi"],
            );
        }
        "audio" => {
            dialog = dialog.add_filter(
                "Audio Files (*.mp3, *.wav, *.flac, *.m4a, *.ogg)",
                &["mp3", "wav", "flac", "m4a", "ogg"],
            );
        }
        "image" | "character" => {
            dialog = dialog.add_filter(
                "Image Files (*.png, *.jpg, *.jpeg, *.webp)",
                &["png", "jpg", "jpeg", "webp"],
            );
        }
        "media_or_image" | "background" => {
            dialog = dialog.add_filter(
                "Media or Image (*.png, *.jpg, *.jpeg, *.webp, *.mp4, *.mkv, *.webm, *.mov)",
                &["png", "jpg", "jpeg", "webp", "mp4", "mkv", "webm", "mov", "avi"],
            );
        }
        _ => {
            dialog = dialog.add_filter(
                "Media Files",
                &[
                    "mp4", "mkv", "webm", "mov", "avi", "mp3", "wav", "flac", "m4a", "ogg", "png", "jpg", "jpeg", "webp",
                ],
            );
        }
    }
    let file = dialog.pick_file();
    Ok(file.map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn probe_media(app: tauri::AppHandle, file_path: String) -> Result<MediaInfo, String> {
    let ffprobe_bin = get_ffprobe_binary(&app).ok();
    probe_media_internal(&file_path, ffprobe_bin.as_deref())
}

pub fn probe_media_internal(file_path: &str, ffprobe_bin: Option<&std::path::Path>) -> Result<MediaInfo, String> {
    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. MP4 / MOV containers (via mp4 crate)
    if ext == "mp4" || ext == "mov" || ext == "m4a" {
        if let Ok(file) = std::fs::File::open(&file_path) {
            let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
            let reader = std::io::BufReader::new(file);
            if let Ok(mp4_reader) = mp4::Mp4Reader::read_header(reader, file_size) {
                let mut duration = mp4_reader.duration().as_secs_f64();
                let mut width = 0u32;
                let mut height = 0u32;
                let mut fps = 0.0;
                let mut audio_channels = 0u32;
                let mut audio_sample_rate = 0u32;

                for track in mp4_reader.tracks().values() {
                    match track.track_type() {
                        Ok(mp4::TrackType::Video) if width == 0 => {
                            width = track.width() as u32;
                            height = track.height() as u32;
                            let sample_count = track.sample_count();
                            let track_dur = track.duration().as_secs_f64();
                            if track_dur > 0.0 && sample_count > 0 {
                                fps = (sample_count as f64) / track_dur;
                                if (fps - fps.round()).abs() < 0.05 {
                                    fps = fps.round();
                                }
                            }
                            if duration == 0.0 {
                                duration = track_dur;
                            }
                        }
                        Ok(mp4::TrackType::Audio) if audio_channels == 0 => {
                            if let Some(mp4a) = track.trak.mdia.minf.stbl.stsd.mp4a.as_ref() {
                                audio_channels = mp4a.channelcount as u32;
                            }
                            if audio_sample_rate == 0 {
                                audio_sample_rate = track.timescale();
                            }
                        }
                        _ => {}
                    }
                }

                // Complement with symphonia for audio if needed
                if let Ok(audio_info) = probe_audio_symphonia(&file_path, &ext) {
                    if audio_channels == 0 {
                        audio_channels = audio_info.audio_channels;
                    }
                    if audio_sample_rate == 0 {
                        audio_sample_rate = audio_info.audio_sample_rate;
                    }
                    if duration == 0.0 {
                        duration = audio_info.duration;
                    }
                }

                if width > 0 || audio_channels > 0 || duration > 0.0 {
                    return Ok(MediaInfo {
                        duration,
                        fps,
                        width,
                        height,
                        audio_channels,
                        audio_sample_rate,
                    });
                }
            }
        }
    }

    // 2. ffprobe fallback for video files when pure-Rust probe missed dimensions
    let video_exts = ["mp4", "mov", "avi", "mkv", "webm", "wmv", "flv", "ts", "m2ts", "mxf"];
    if video_exts.contains(&ext.as_str()) {
        if let Some(ffprobe_bin) = ffprobe_bin {
            #[cfg(target_os = "windows")]
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new(ffprobe_bin);
            cmd.args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height,avg_frame_rate,r_frame_rate,duration",
                "-of", "json",
                file_path,
            ]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        if let Some(streams) = json["streams"].as_array() {
                            if let Some(stream) = streams.first() {
                                let width = stream["width"].as_u64().unwrap_or(0) as u32;
                                let height = stream["height"].as_u64().unwrap_or(0) as u32;
                                let mut fps = 0.0f64;
                                // Try avg_frame_rate first, then r_frame_rate
                                for key in ["avg_frame_rate", "r_frame_rate"] {
                                    if let Some(rate_str) = stream[key].as_str() {
                                        if let Some((num, den)) = rate_str.split_once('/') {
                                            if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                                                if d > 0.0 {
                                                    fps = n / d;
                                                    if (fps - fps.round()).abs() < 0.05 {
                                                        fps = fps.round();
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                let duration_str = stream["duration"].as_str().unwrap_or("0");
                                let probe_dur = duration_str.parse::<f64>().unwrap_or(0.0);
                                // Complement with symphonia for audio info
                                let (audio_ch, audio_sr, sym_dur) = if let Ok(ai) = probe_audio_symphonia(file_path, &ext) {
                                    (ai.audio_channels, ai.audio_sample_rate, ai.duration)
                                } else { (0, 0, 0.0) };
                                let final_dur = if probe_dur > 0.0 { probe_dur } else { sym_dur };
                                if width > 0 && height > 0 {
                                    return Ok(MediaInfo {
                                        duration: final_dur,
                                        fps,
                                        width,
                                        height,
                                        audio_channels: audio_ch,
                                        audio_sample_rate: audio_sr,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        // If ffprobe also failed for a video file, return specific error
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or(file_path);
        return Err(format!(
            "Could not probe video dimensions for '{}' ({} container). Try re-encoding to H.264 MP4.",
            filename, ext
        ));
    }

    // 3. Audio and other containers (via symphonia crate)
    probe_audio_symphonia(file_path, &ext)
}

pub fn probe_audio_symphonia(file_path: &str, ext: &str) -> Result<MediaInfo, String> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open file '{file_path}': {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| format!("Unsupported format '{ext}': {e}"))?;

    let mut format = probed.format;
    let mut duration = 0.0;
    let mut audio_channels = 0u32;
    let mut audio_sample_rate = 0u32;

    for track in format.tracks() {
        let params = &track.codec_params;
        if let Some(sr) = params.sample_rate {
            if audio_sample_rate == 0 {
                audio_sample_rate = sr;
            }
        }
        if let Some(ch) = params.channels {
            if audio_channels == 0 {
                audio_channels = ch.count() as u32;
            }
        }
        if let (Some(n_frames), Some(tb)) = (params.n_frames, params.time_base) {
            let dur = tb.calc_time(n_frames);
            let d = dur.seconds as f64 + dur.frac;
            if d > duration {
                duration = d;
            }
        } else if let (Some(n_frames), Some(sr)) = (params.n_frames, params.sample_rate) {
            if sr > 0 {
                let d = (n_frames as f64) / (sr as f64);
                if d > duration {
                    duration = d;
                }
            }
        }
    }

    // Fallback: iterate packets to calculate duration if not present in header
    if duration == 0.0 {
        let default_track = format.default_track().cloned();
        if let Some(track) = default_track {
            let tb = track.codec_params.time_base;
            let sr = track.codec_params.sample_rate.unwrap_or(44100);
            let mut total_ts = 0u64;
            while let Ok(packet) = format.next_packet() {
                if packet.track_id() == track.id {
                    total_ts += packet.dur();
                }
            }
            if let Some(tb) = tb {
                let dur = tb.calc_time(total_ts);
                duration = dur.seconds as f64 + dur.frac;
            } else if sr > 0 {
                duration = (total_ts as f64) / (sr as f64);
            }
        }
    }

    Ok(MediaInfo {
        duration,
        fps: 0.0,
        width: 0,
        height: 0,
        audio_channels,
        audio_sample_rate,
    })
}
