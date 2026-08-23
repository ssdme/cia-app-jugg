use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::dumper::DumpAnalysis;
use crate::nle::compute_crc32;
use crate::probe::probe_media_internal;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub quick_hash: String,
    pub absolute_path: String,
    pub metadata: MediaMetadata,
    pub analysis: Option<DumpAnalysis>,
    pub last_used: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaPoolIndex {
    pub schema_version: u32,
    pub assets: HashMap<String, MediaAsset>,
}

impl Default for MediaPoolIndex {
    fn default() -> Self {
        Self {
            schema_version: 1,
            assets: HashMap::new(),
        }
    }
}

/// Compute Quick Hash: size + mtime + first 64KB + last 64KB
pub fn compute_quick_hash(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    
    let file_size = metadata.len();
    let mtime_secs = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;

    let sample_size = 65536usize; // 64 KB

    // Head sample
    let mut head_buf = vec![0u8; (file_size as usize).min(sample_size)];
    let head_read = file.read(&mut head_buf)
        .map_err(|e| format!("Failed to read head sample: {e}"))?;
    head_buf.truncate(head_read);

    // Tail sample
    let mut tail_buf = Vec::new();
    if file_size > (sample_size as u64) {
        let tail_start = file_size.saturating_sub(sample_size as u64);
        if file.seek(SeekFrom::Start(tail_start)).is_ok() {
            let mut buf = vec![0u8; sample_size];
            if let Ok(n) = file.read(&mut buf) {
                buf.truncate(n);
                tail_buf = buf;
            }
        }
    }

    let mut combined = Vec::with_capacity(16 + head_buf.len() + tail_buf.len());
    combined.extend_from_slice(&file_size.to_le_bytes());
    combined.extend_from_slice(&mtime_secs.to_le_bytes());
    combined.extend_from_slice(&head_buf);
    combined.extend_from_slice(&tail_buf);

    let crc1 = compute_crc32(&combined);
    let crc2 = compute_crc32(&head_buf);
    let crc3 = compute_crc32(&tail_buf);

    Ok(format!("{:08x}{:08x}{:08x}_{:x}", crc1, crc2, crc3, file_size))
}

pub fn get_media_pool_index_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = Path::new(&appdata).join("cia_app");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("media_pool_index.json")
    } else {
        let dir = std::env::temp_dir().join("cia_app");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("media_pool_index.json")
    }
}

pub fn load_media_pool_from_disk(path: &Path) -> MediaPoolIndex {
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(index) = serde_json::from_str::<MediaPoolIndex>(&content) {
                return index;
            }
        }
    }
    MediaPoolIndex::default()
}

pub fn save_media_pool_to_disk(index: &MediaPoolIndex, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize media pool index: {e}"))?;
    std::fs::write(path, json)
        .map_err(|e| format!("Failed to write media pool index: {e}"))?;
    Ok(())
}

#[derive(Clone)]
pub struct MediaPoolManager {
    pub index: Arc<Mutex<MediaPoolIndex>>,
    pub index_file_path: PathBuf,
}

impl Default for MediaPoolManager {
    fn default() -> Self {
        let index_path = get_media_pool_index_path();
        let index = load_media_pool_from_disk(&index_path);
        Self {
            index: Arc::new(Mutex::new(index)),
            index_file_path: index_path,
        }
    }
}

impl MediaPoolManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_custom_path(path: PathBuf) -> Self {
        let index = load_media_pool_from_disk(&path);
        Self {
            index: Arc::new(Mutex::new(index)),
            index_file_path: path,
        }
    }

    pub fn get_asset(&self, quick_hash: &str) -> Option<MediaAsset> {
        self.index.lock().unwrap().assets.get(quick_hash).cloned()
    }

    pub fn find_by_path(&self, abs_path: &str) -> Option<MediaAsset> {
        let path = Path::new(abs_path);
        if let Ok(hash) = compute_quick_hash(path) {
            self.get_asset(&hash)
        } else {
            self.index.lock().unwrap().assets.values().find(|a| a.absolute_path == abs_path).cloned()
        }
    }

    pub fn insert_asset(&self, asset: MediaAsset) -> Result<(), String> {
        {
            let mut index = self.index.lock().unwrap();
            index.assets.insert(asset.quick_hash.clone(), asset);
        }
        let index_lock = self.index.lock().unwrap();
        save_media_pool_to_disk(&index_lock, &self.index_file_path)
    }

    pub fn remove_asset(&self, quick_hash: &str) -> Result<bool, String> {
        let removed = {
            let mut index = self.index.lock().unwrap();
            index.assets.remove(quick_hash).is_some()
        };
        if removed {
            let index_lock = self.index.lock().unwrap();
            save_media_pool_to_disk(&index_lock, &self.index_file_path)?;
        }
        Ok(removed)
    }

    pub fn list_assets(&self) -> Vec<MediaAsset> {
        let mut list: Vec<MediaAsset> = self.index.lock().unwrap().assets.values().cloned().collect();
        list.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        list
    }
}

static GLOBAL_MEDIA_POOL_MANAGER: std::sync::OnceLock<MediaPoolManager> = std::sync::OnceLock::new();

pub fn global_media_pool_manager() -> &'static MediaPoolManager {
    GLOBAL_MEDIA_POOL_MANAGER.get_or_init(MediaPoolManager::new)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_media_pool() -> Result<Vec<MediaAsset>, String> {
    Ok(global_media_pool_manager().list_assets())
}

#[tauri::command]
pub fn remove_media_from_pool(hash: String) -> Result<bool, String> {
    global_media_pool_manager().remove_asset(&hash)
}

#[tauri::command]
pub fn get_cached_analysis_for_media(path: String) -> Result<Option<DumpAnalysis>, String> {
    let mgr = global_media_pool_manager();
    if let Some(asset) = mgr.find_by_path(&path) {
        Ok(asset.analysis)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn import_media_to_pool(paths: Vec<String>) -> Result<Vec<MediaAsset>, String> {
    let mgr = global_media_pool_manager();
    let mut imported = Vec::with_capacity(paths.len());

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    for p_str in paths {
        let p = Path::new(&p_str);
        if !p.exists() {
            continue;
        }

        let hash = match compute_quick_hash(p) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[MediaPool] Hash error for {}: {e}", p.display());
                continue;
            }
        };

        // Cache Hit check
        if let Some(mut existing) = mgr.get_asset(&hash) {
            existing.last_used = now_ts;
            let _ = mgr.insert_asset(existing.clone());
            imported.push(existing);
            continue;
        }

        // Cache Miss -> probe & populate
        let file_name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file_size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);

        let probed = probe_media_internal(&p_str, None).ok();
        let (duration, width, height, fps, codec) = if let Some(ref pr) = probed {
            (
                pr.duration,
                pr.width,
                pr.height,
                pr.fps,
                "h264".to_string(),
            )
        } else {
            (10.0, 1080, 1080, 30.0, "unknown".to_string())
        };

        let metadata = MediaMetadata {
            file_name,
            file_size_bytes: file_size,
            duration,
            width,
            height,
            fps,
            codec,
        };

        let asset = MediaAsset {
            quick_hash: hash,
            absolute_path: p_str.clone(),
            metadata,
            analysis: None,
            last_used: now_ts,
        };

        let _ = mgr.insert_asset(asset.clone());
        imported.push(asset);
    }

    Ok(imported)
}
