use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::export::ExportSettings;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BatchAction {
    AnalyzeOnly,
    AnalyzeAndRender,
    AnalyzeAndExportNLE,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchConfig {
    pub source_dir: String,
    #[serde(default = "default_extensions")]
    pub file_extensions: Vec<String>,
    #[serde(default = "default_batch_action")]
    pub action: BatchAction,
    pub preset_name: Option<String>,
    pub export_settings: Option<ExportSettings>,
    #[serde(default = "default_concurrency")]
    pub concurrency_limit: usize,
}

fn default_extensions() -> Vec<String> {
    vec!["mp4".to_string(), "mov".to_string(), "mkv".to_string(), "webm".to_string(), "avi".to_string()]
}
fn default_batch_action() -> BatchAction { BatchAction::AnalyzeAndRender }
fn default_concurrency() -> usize { 2 }

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            source_dir: "".to_string(),
            file_extensions: default_extensions(),
            action: default_batch_action(),
            preset_name: Some("AGGRESSIVE_JUGG".to_string()),
            export_settings: Some(ExportSettings::default()),
            concurrency_limit: 2,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchItemStatus {
    Pending,
    Processing,
    Success,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemResult {
    pub file_path: String,
    pub file_name: String,
    pub status: BatchItemStatus,
    pub output_path: Option<String>,
    pub duration_secs: f64,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchJobStatus {
    Pending,
    Running,
    Completed,
    PartialSuccess,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatchJob {
    pub id: String,
    pub config: BatchConfig,
    pub total_files: usize,
    pub completed_files: usize,
    pub status: BatchJobStatus,
    pub items: Vec<BatchItemResult>,
    pub report_path: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
}

pub fn scan_directory_recursive(dir: &Path, extensions: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() || !dir.is_dir() {
        return files;
    }

    let lower_exts: Vec<String> = extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .collect();

    let mut stack = vec![dir.to_path_buf()];
    while let Some(current_dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if lower_exts.contains(&ext_str) {
                            files.push(path);
                        }
                    }
                }
            }
        }
    }

    files.sort();
    files
}

pub fn generate_batch_report_markdown(job: &BatchJob) -> String {
    let success_count = job.items.iter().filter(|i| i.status == BatchItemStatus::Success).count();
    let failed_count = job.items.iter().filter(|i| i.status == BatchItemStatus::Failed).count();
    let total_count = job.items.len();

    let mut md = String::new();
    md.push_str(&format!("# CIA App Jugg — Batch Processing Report\n\n"));
    md.push_str(&format!("- **Batch ID**: `{}`\n", job.id));
    md.push_str(&format!("- **Source Directory**: `{}`\n", job.config.source_dir));
    md.push_str(&format!("- **Action**: `{:?}`\n", job.config.action));
    md.push_str(&format!("- **Preset**: `{}`\n", job.config.preset_name.as_deref().unwrap_or("None")));
    md.push_str(&format!("- **Total Files**: `{}`\n", total_count));
    md.push_str(&format!("- **Success**: `{}`\n", success_count));
    md.push_str(&format!("- **Failed**: `{}`\n\n", failed_count));

    md.push_str("## Processed Files\n\n");
    md.push_str("| File Name | Status | Duration (s) | Output / Details |\n");
    md.push_str("| :--- | :---: | :---: | :--- |\n");

    for item in &job.items {
        let status_badge = match item.status {
            BatchItemStatus::Success => "âœ… SUCCESS",
            BatchItemStatus::Failed => "âŒ FAILED",
            BatchItemStatus::Processing => "âš¡ PROCESSING",
            BatchItemStatus::Pending => "â³ PENDING",
        };
        let details = if let Some(out) = &item.output_path {
            format!("`{}`", out)
        } else if let Some(err) = &item.error_message {
            format!("*Error: {}*", err)
        } else {
            "-".to_string()
        };

        md.push_str(&format!(
            "| `{}` | {} | {:.2} | {} |\n",
            item.file_name, status_badge, item.duration_secs, details
        ));
    }

    if failed_count > 0 {
        md.push_str("\n## Errors Summary\n\n");
        for item in &job.items {
            if item.status == BatchItemStatus::Failed {
                md.push_str(&format!(
                    "- **`{}`**: {}\n",
                    item.file_name,
                    item.error_message.as_deref().unwrap_or("Unknown failure")
                ));
            }
        }
    }

    md
}

#[derive(Clone)]
pub struct BatchManager {
    pub jobs: Arc<Mutex<HashMap<String, BatchJob>>>,
}

impl Default for BatchManager {
    fn default() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

static GLOBAL_BATCH_MANAGER: std::sync::OnceLock<BatchManager> = std::sync::OnceLock::new();

pub fn global_batch_manager() -> &'static BatchManager {
    GLOBAL_BATCH_MANAGER.get_or_init(BatchManager::default)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn start_batch_job(config: BatchConfig) -> Result<String, String> {
    let dir = Path::new(&config.source_dir);
    if !dir.exists() {
        return Err(format!("Source directory does not exist: {}", config.source_dir));
    }

    let files = scan_directory_recursive(dir, &config.file_extensions);
    if files.is_empty() {
        return Err(format!("No matching media files found in {}", config.source_dir));
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let batch_id = format!("batch_{timestamp}");

    let mut items = Vec::with_capacity(files.len());
    for f in &files {
        items.push(BatchItemResult {
            file_path: f.to_string_lossy().to_string(),
            file_name: f.file_name().unwrap_or_default().to_string_lossy().to_string(),
            status: BatchItemStatus::Pending,
            output_path: None,
            duration_secs: 0.0,
            error_message: None,
        });
    }

    let job = BatchJob {
        id: batch_id.clone(),
        config: config.clone(),
        total_files: files.len(),
        completed_files: 0,
        status: BatchJobStatus::Running,
        items,
        report_path: None,
        start_time: timestamp,
        end_time: None,
    };

    let manager = global_batch_manager();
    manager.jobs.lock().unwrap().insert(batch_id.clone(), job);

    // Spawn background batch executor
    let b_id = batch_id.clone();
    let concurrency = config.concurrency_limit.clamp(1, 4);

    std::thread::spawn(move || {
        let m = global_batch_manager();

        // Process files in batches matching concurrency limit
        for chunk in files.chunks(concurrency) {
            let mut handles = Vec::new();
            for file_path in chunk {
                let f_path = file_path.clone();
                let b_id_c = b_id.clone();
                let f_str = f_path.to_string_lossy().to_string();

                let h = std::thread::spawn(move || {
                    let start = std::time::Instant::now();
                    // Mock / process step (e.g. analysis & remap)
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let elapsed = start.elapsed().as_secs_f64();
                    (b_id_c, f_str, elapsed, true, None)
                });
                handles.push(h);
            }

            for h in handles {
                if let Ok((job_id, f_path, duration, success, err_msg)) = h.join() {
                    if let Ok(mut map) = m.jobs.lock() {
                        if let Some(j) = map.get_mut(&job_id) {
                            if let Some(item) = j.items.iter_mut().find(|i| i.file_path == f_path) {
                                if success {
                                    item.status = BatchItemStatus::Success;
                                    item.output_path = Some(format!("{}_jugg_out.mp4", f_path));
                                } else {
                                    item.status = BatchItemStatus::Failed;
                                    item.error_message = err_msg;
                                }
                                item.duration_secs = duration;
                            }
                            j.completed_files += 1;
                        }
                    }
                }
            }
        }

        // Finalize batch job & generate report
        if let Ok(mut map) = m.jobs.lock() {
            if let Some(j) = map.get_mut(&b_id) {
                let has_failed = j.items.iter().any(|i| i.status == BatchItemStatus::Failed);
                let all_failed = j.items.iter().all(|i| i.status == BatchItemStatus::Failed);

                j.status = if all_failed {
                    BatchJobStatus::Failed
                } else if has_failed {
                    BatchJobStatus::PartialSuccess
                } else {
                    BatchJobStatus::Completed
                };

                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                j.end_time = Some(now_ts);

                let report_content = generate_batch_report_markdown(j);
                let report_file = Path::new(&j.config.source_dir).join(format!("batch_report_{now_ts}.md"));
                let _ = std::fs::write(&report_file, report_content);
                j.report_path = Some(report_file.to_string_lossy().to_string());
            }
        }
    });

    Ok(batch_id)
}

#[tauri::command]
pub fn get_batch_status(batch_id: String) -> Result<Option<BatchJob>, String> {
    let manager = global_batch_manager();
    Ok(manager.jobs.lock().unwrap().get(&batch_id).cloned())
}

#[tauri::command]
pub fn list_batch_jobs() -> Result<Vec<BatchJob>, String> {
    let manager = global_batch_manager();
    let jobs = manager.jobs.lock().unwrap().values().cloned().collect();
    Ok(jobs)
}
