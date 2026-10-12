use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};

type ActiveJob = Option<(String, Arc<AtomicBool>)>;
static ACTIVE: OnceLock<Mutex<ActiveJob>> = OnceLock::new();
fn registry() -> &'static Mutex<ActiveJob> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

pub struct RenderJob {
    pub id: String,
    pub cancelled: Arc<AtomicBool>,
}
impl RenderJob {
    pub fn begin(id: String) -> Result<Self, String> {
        if id.is_empty()
            || id.len() > 100
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Invalid render job ID".into());
        }
        let mut active = registry()
            .lock()
            .map_err(|_| "Render registry unavailable")?;
        if active.is_some() {
            return Err("A render is still active or cancelling".into());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        *active = Some((id.clone(), cancelled.clone()));
        Ok(Self { id, cancelled })
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Acquire) {
            Err("Render cancelled by user".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for RenderJob {
    fn drop(&mut self) {
        if let Ok(mut active) = registry().lock() {
            if active.as_ref().is_some_and(|(id, _)| id == &self.id) {
                *active = None;
            }
        }
    }
}
pub fn cancel(id: &str) -> Result<(), String> {
    let active = registry()
        .lock()
        .map_err(|_| "Render registry unavailable")?;
    if let Some((active_id, flag)) = &*active {
        if active_id == id {
            flag.store(true, Ordering::Release);
        } else {
            return Err("Cancellation belongs to a different render job".into());
        }
    }
    Ok(()) // Cancellation during frontend plan generation is handled before launch.
}

pub struct JobDirectory(pub std::path::PathBuf);
impl JobDirectory {
    pub fn create(root: &std::path::Path, id: &str) -> Result<Self, String> {
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let path = root.join(format!("render-{}-{id}", std::process::id()));
        std::fs::create_dir(&path).map_err(|e| format!("Cannot create render cache: {e}"))?;
        Ok(Self(path))
    }
}
impl Drop for JobDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_exclusion_and_cancellation_are_scoped() {
        let first = RenderJob::begin("registry-test".into()).unwrap();
        assert!(RenderJob::begin("second".into()).is_err());
        assert!(cancel("stale").is_err());
        assert!(first.check().is_ok());
        cancel(&first.id).unwrap();
        assert!(first.check().is_err());
        drop(first);
        assert!(RenderJob::begin("restart".into()).is_ok());
    }
}
