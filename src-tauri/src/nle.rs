use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::presets::ProjectState;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BeatMarkerItem {
    pub time: f64,
    #[serde(rename = "type")]
    pub marker_type: String, // "downbeat" | "beat"
    pub bpm: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TimeRemapItem {
    pub target_start: f64,
    pub target_end: f64,
    pub source_start: f64,
    pub source_end: f64,
    pub curve_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NlePackageData {
    pub project_name: String,
    pub bpm: f64,
    pub fps: u32,
    pub total_duration: f64,
    pub beat_grid: Vec<BeatMarkerItem>,
    pub time_remap: Vec<TimeRemapItem>,
}

/// Calculate IEEE 802.3 CRC-32 checksum
pub fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Create a standard ZIP archive from in-memory (filename, content) pairs
pub fn create_zip_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip_data = Vec::new();
    let mut central_directory = Vec::new();

    for &(filename, content) in files {
        let local_header_offset = zip_data.len() as u32;
        let fname_bytes = filename.as_bytes();
        let fname_len = fname_bytes.len() as u16;
        let crc = compute_crc32(content);
        let size = content.len() as u32;

        // Local file header (30 bytes + filename)
        zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes()); // signature
        zip_data.extend_from_slice(&20u16.to_le_bytes());         // version needed: 2.0
        zip_data.extend_from_slice(&0u16.to_le_bytes());          // flags
        zip_data.extend_from_slice(&0u16.to_le_bytes());          // compression: Store (0)
        zip_data.extend_from_slice(&0u16.to_le_bytes());          // mod time
        zip_data.extend_from_slice(&0u16.to_le_bytes());          // mod date
        zip_data.extend_from_slice(&crc.to_le_bytes());           // CRC-32
        zip_data.extend_from_slice(&size.to_le_bytes());          // compressed size
        zip_data.extend_from_slice(&size.to_le_bytes());          // uncompressed size
        zip_data.extend_from_slice(&fname_len.to_le_bytes());     // filename length
        zip_data.extend_from_slice(&0u16.to_le_bytes());          // extra field len
        zip_data.extend_from_slice(fname_bytes);                  // filename
        zip_data.extend_from_slice(content);                      // uncompressed data

        // Central Directory Entry (46 bytes + filename)
        central_directory.extend_from_slice(&0x02014b50u32.to_le_bytes()); // signature
        central_directory.extend_from_slice(&20u16.to_le_bytes());         // version made by
        central_directory.extend_from_slice(&20u16.to_le_bytes());         // version needed
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // flags
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // compression
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // mod time
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // mod date
        central_directory.extend_from_slice(&crc.to_le_bytes());           // CRC-32
        central_directory.extend_from_slice(&size.to_le_bytes());          // compressed size
        central_directory.extend_from_slice(&size.to_le_bytes());          // uncompressed size
        central_directory.extend_from_slice(&fname_len.to_le_bytes());     // filename length
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // extra field len
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // file comment len
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // disk number start
        central_directory.extend_from_slice(&0u16.to_le_bytes());          // internal attrs
        central_directory.extend_from_slice(&0u32.to_le_bytes());          // external attrs
        central_directory.extend_from_slice(&local_header_offset.to_le_bytes()); // offset
        central_directory.extend_from_slice(fname_bytes);
    }

    let cd_offset = zip_data.len() as u32;
    let cd_size = central_directory.len() as u32;
    let entries_count = files.len() as u16;

    zip_data.extend_from_slice(&central_directory);

    // End of Central Directory Record (22 bytes)
    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes()); // signature
    zip_data.extend_from_slice(&0u16.to_le_bytes());          // disk number
    zip_data.extend_from_slice(&0u16.to_le_bytes());          // disk with CD
    zip_data.extend_from_slice(&entries_count.to_le_bytes()); // entries on this disk
    zip_data.extend_from_slice(&entries_count.to_le_bytes()); // total entries
    zip_data.extend_from_slice(&cd_size.to_le_bytes());       // size of CD
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());     // offset of CD
    zip_data.extend_from_slice(&0u16.to_le_bytes());          // comment length

    zip_data
}

/// Parse ZIP archive and return entries map (filename -> data)
pub fn read_zip_entries(zip_bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut map = HashMap::new();
    let mut cursor = 0;
    while cursor + 30 <= zip_bytes.len() {
        let sig = u32::from_le_bytes([zip_bytes[cursor], zip_bytes[cursor+1], zip_bytes[cursor+2], zip_bytes[cursor+3]]);
        if sig != 0x04034b50 {
            break;
        }
        let fname_len = u16::from_le_bytes([zip_bytes[cursor+26], zip_bytes[cursor+27]]) as usize;
        let extra_len = u16::from_le_bytes([zip_bytes[cursor+28], zip_bytes[cursor+29]]) as usize;
        let comp_size = u32::from_le_bytes([zip_bytes[cursor+18], zip_bytes[cursor+19], zip_bytes[cursor+20], zip_bytes[cursor+21]]) as usize;

        let name_start = cursor + 30;
        let name_end = name_start + fname_len;
        if name_end > zip_bytes.len() {
            return Err("Malformed ZIP filename".to_string());
        }
        let filename = String::from_utf8_lossy(&zip_bytes[name_start..name_end]).to_string();

        let data_start = name_end + extra_len;
        let data_end = data_start + comp_size;
        if data_end > zip_bytes.len() {
            return Err("Malformed ZIP data bounds".to_string());
        }
        let data = zip_bytes[data_start..data_end].to_vec();
        map.insert(filename, data);

        cursor = data_end;
    }
    Ok(map)
}

pub fn generate_after_effects_jsx_script(package_data: &NlePackageData) -> String {
    let json_str = serde_json::to_string_pretty(package_data).unwrap_or_else(|_| "{}".to_string());

    format!(r#"/**
 * CIA App Jugg — After Effects Beat Grid & Time-Remap Automation Script
 * Generated automatically for project: {project_name}
 * BPM: {bpm:.1} | Total Duration: {duration:.2}s | FPS: {fps}
 */

(function() {{
    app.beginUndoGroup("Create Jugg Markers & Remap");

    var juggData = {json_str};

    var comp = app.project.activeItem;
    if (!comp || !(comp instanceof CompItem)) {{
        alert("Error: Please select or open an active composition in After Effects first.");
        return;
    }}

    // 1. Add Beat Grid Markers to Composition
    var markersAdded = 0;
    if (juggData.beat_grid && juggData.beat_grid.length > 0) {{
        for (var i = 0; i < juggData.beat_grid.length; i++) {{
            var item = juggData.beat_grid[i];
            var markerVal = new MarkerValue(item.type === "downbeat" ? "DOWNBEAT" : "BEAT");
            if (item.type === "downbeat") {{
                markerVal.label = 1; // Red label in After Effects
            }} else {{
                markerVal.label = 8; // Gray / Default label
            }}
            comp.markerProperty.setValueAtTime(item.time, markerVal);
            markersAdded++;
        }}
    }}

    // 2. Optional: Apply Time-Remap keyframes to selected layer
    var selectedLayers = comp.selectedLayers;
    if (selectedLayers && selectedLayers.length > 0 && juggData.time_remap && juggData.time_remap.length > 0) {{
        var layer = selectedLayers[0];
        if (layer.canSetTimeRemapEnabled) {{
            layer.timeRemapEnabled = true;
            var timeRemapProp = layer.property("Time Remap");
            
            // Clear existing keyframes
            while (timeRemapProp.numKeys > 0) {{
                timeRemapProp.removeKey(1);
            }}

            for (var k = 0; k < juggData.time_remap.length; k++) {{
                var seg = juggData.time_remap[k];
                timeRemapProp.setValueAtTime(seg.target_start, seg.source_start);
                timeRemapProp.setValueAtTime(seg.target_end, seg.source_end);
            }}
        }}
    }}

    app.endUndoGroup();
    alert("Jugg markers created successfully! (" + markersAdded + " markers added)");
}})();
"#,
        project_name = package_data.project_name,
        bpm = package_data.bpm,
        duration = package_data.total_duration,
        fps = package_data.fps,
        json_str = json_str
    )
}

pub fn build_nle_package_zip(project: &ProjectState) -> Result<Vec<u8>, String> {
    let bpm = project.plan.as_ref().map(|p| p.bpm).unwrap_or(120.0);
    let fps = project.plan.as_ref().map(|p| p.fps).unwrap_or(30);
    let target_dur = project.plan.as_ref().map(|p| p.target_duration).unwrap_or(5.0);

    let mut beat_grid = Vec::new();
    let mut time_remap = Vec::new();

    if let Some(plan) = &project.plan {
        // Collect segments
        for seg in &plan.segments {
            time_remap.push(TimeRemapItem {
                target_start: seg.t0,
                target_end: seg.t1,
                source_start: seg.s0,
                source_end: seg.s1,
                curve_type: seg.curve.clone(),
            });
        }

        // Build beat markers from segments and target duration
        let beat_interval = 60.0 / bpm as f64;
        let mut t = 0.0;
        while t <= target_dur {
            let is_downbeat = plan.segments.iter().any(|s| (s.t0 - t).abs() < 0.05);
            beat_grid.push(BeatMarkerItem {
                time: (t * 1000.0).round() / 1000.0,
                marker_type: if is_downbeat { "downbeat".to_string() } else { "beat".to_string() },
                bpm,
            });
            t += beat_interval;
        }
    }

    let package_data = NlePackageData {
        project_name: project.project_name.clone().unwrap_or_else(|| "CIA_Jugg_Project".to_string()),
        bpm,
        fps,
        total_duration: target_dur,
        beat_grid: beat_grid.clone(),
        time_remap: time_remap.clone(),
    };

    let beat_grid_json = serde_json::to_vec_pretty(&beat_grid)
        .map_err(|e| format!("Serialization error beat_grid: {e}"))?;
    let time_remap_json = serde_json::to_vec_pretty(&time_remap)
        .map_err(|e| format!("Serialization error time_remap: {e}"))?;
    let jsx_script = generate_after_effects_jsx_script(&package_data);

    let files: [(&str, &[u8]); 3] = [
        ("beat_grid.json", &beat_grid_json),
        ("time_remap.json", &time_remap_json),
        ("Create_Jugg_Markers.jsx", jsx_script.as_bytes()),
    ];

    Ok(create_zip_archive(&files))
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn export_for_nle(project: ProjectState, output_dir: String) -> Result<String, String> {
    let zip_bytes = build_nle_package_zip(&project)?;
    let out_dir_path = Path::new(&output_dir);
    if !out_dir_path.exists() {
        std::fs::create_dir_all(out_dir_path)
            .map_err(|e| format!("Failed to create output dir: {e}"))?;
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filename = format!("cia_nle_export_{timestamp}.zip");
    let target_path = out_dir_path.join(filename);
    std::fs::write(&target_path, zip_bytes)
        .map_err(|e| format!("Failed to write zip file: {e}"))?;

    Ok(target_path.to_string_lossy().to_string())
}
