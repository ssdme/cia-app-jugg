//! Bound native analysis work without occupying the window/event-loop thread.
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::{Duration, UNIX_EPOCH},
};
static RUNNING: Mutex<usize> = Mutex::new(0);
static AVAILABLE: Condvar = Condvar::new();
pub struct AnalysisPermit;
impl AnalysisPermit {
    pub fn acquire() -> Result<Self, String> {
        Self::acquire_cancelled(&AtomicBool::new(false))
    }
    pub fn acquire_cancelled(cancelled: &AtomicBool) -> Result<Self, String> {
        let mut running = RUNNING.lock().map_err(|_| "Analysis queue unavailable")?;
        while *running >= 2 {
            check_cancelled(cancelled)?;
            running = AVAILABLE
                .wait_timeout(running, Duration::from_millis(100))
                .map_err(|_| "Analysis queue unavailable")?
                .0;
        }
        check_cancelled(cancelled)?;
        *running += 1;
        Ok(Self)
    }
}

#[derive(Default)]
struct AnalysisJobs {
    active: HashMap<String, Arc<AtomicBool>>,
    pending_cancels: VecDeque<String>,
}
type AnalysisRegistry = Mutex<AnalysisJobs>;
fn registry() -> &'static AnalysisRegistry {
    static JOBS: OnceLock<AnalysisRegistry> = OnceLock::new();
    JOBS.get_or_init(|| Mutex::new(AnalysisJobs::default()))
}
pub struct AnalysisJob {
    pub id: String,
    pub cancelled: Arc<AtomicBool>,
}
impl AnalysisJob {
    pub fn begin(id: String) -> Result<Self, String> {
        if id.is_empty()
            || id.len() > 100
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Invalid analysis job ID".into());
        }
        let mut jobs = registry()
            .lock()
            .map_err(|_| "Analysis registry unavailable")?;
        if let Some(index) = jobs
            .pending_cancels
            .iter()
            .position(|pending| pending == &id)
        {
            jobs.pending_cancels.remove(index);
            return Err("Analysis cancelled by user".into());
        }
        if jobs.active.contains_key(&id) || jobs.active.len() >= 16 {
            return Err("Analysis job is already active or the queue is full".into());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        jobs.active.insert(id.clone(), cancelled.clone());
        Ok(Self { id, cancelled })
    }
}
impl Drop for AnalysisJob {
    fn drop(&mut self) {
        if let Ok(mut jobs) = registry().lock() {
            jobs.active.remove(&self.id);
        }
    }
}
pub fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Acquire) {
        Err("Analysis cancelled by user".into())
    } else {
        Ok(())
    }
}
pub fn cancel(id: &str) -> Result<(), String> {
    let mut jobs = registry()
        .lock()
        .map_err(|_| "Analysis registry unavailable")?;
    if let Some(flag) = jobs.active.get(id) {
        flag.store(true, Ordering::Release);
    } else if !jobs.pending_cancels.iter().any(|pending| pending == id) {
        jobs.pending_cancels.push_back(id.into());
        if jobs.pending_cancels.len() > 64 {
            jobs.pending_cancels.pop_front();
        }
    }
    Ok(())
}
#[tauri::command]
pub fn cancel_scene_analysis(job_id: String) -> Result<(), String> {
    cancel(&job_id)
}

pub fn fingerprint(path: &Path) -> Result<String, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| format!("Cannot locate media: {e}"))?;
    let metadata = std::fs::metadata(&canonical).map_err(|e| e.to_string())?;
    let modified = metadata
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    Ok(format!(
        "{}:{}:{modified}",
        canonical.display(),
        metadata.len()
    ))
}
impl Drop for AnalysisPermit {
    fn drop(&mut self) {
        if let Ok(mut running) = RUNNING.lock() {
            *running = running.saturating_sub(1);
            AVAILABLE.notify_one();
        }
    }
}
