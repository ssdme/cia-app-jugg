//! Child ownership, bounded stderr and cancellation while a pipe is blocked.
use std::{
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct ManagedChild {
    child: Arc<Mutex<Child>>,
    finished: Arc<AtomicBool>,
    monitor: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    pub stdin: Option<std::process::ChildStdin>,
    pub stdout: Option<std::process::ChildStdout>,
}
pub type StderrObserver = Box<dyn FnMut(&str) + Send>;
impl ManagedChild {
    pub fn spawn(command: &mut Command, cancelled: Arc<AtomicBool>) -> Result<Self, String> {
        Self::spawn_observed(command, cancelled, None)
    }
    pub fn spawn_observed(
        command: &mut Command,
        cancelled: Arc<AtomicBool>,
        mut observer: Option<StderrObserver>,
    ) -> Result<Self, String> {
        command.stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot start media process: {e}"))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take().map(|mut pipe| {
            thread::spawn(move || {
                let mut tail = Vec::new();
                let mut pending = Vec::new();
                let mut buffer = [0u8; 8192];
                while let Ok(count) = pipe.read(&mut buffer) {
                    if count == 0 {
                        break;
                    }
                    if let Some(observer) = observer.as_mut() {
                        for &byte in &buffer[..count] {
                            if byte == b'\n' {
                                observer(&String::from_utf8_lossy(&pending));
                                pending.clear();
                            } else if pending.len() < 65536 {
                                pending.push(byte);
                            }
                        }
                    }
                    tail.extend_from_slice(&buffer[..count]);
                    if tail.len() > 65536 {
                        tail.drain(..tail.len() - 65536);
                    }
                }
                tail
            })
        });
        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));
        let monitor_child = child.clone();
        let monitor_finished = finished.clone();
        let monitor = Some(thread::spawn(move || {
            while !monitor_finished.load(Ordering::Acquire) {
                if cancelled.load(Ordering::Acquire) {
                    if let Ok(mut child) = monitor_child.lock() {
                        let _ = child.kill();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        }));
        Ok(Self {
            child,
            finished,
            monitor,
            stderr,
            stdin,
            stdout,
        })
    }
    pub fn wait(&mut self) -> Result<(ExitStatus, String), String> {
        let status = loop {
            let status = self
                .child
                .lock()
                .map_err(|_| "Child lock unavailable")?
                .try_wait()
                .map_err(|e| format!("Cannot wait for media process: {e}"))?;
            if let Some(status) = status {
                break status;
            }
            thread::sleep(Duration::from_millis(10));
        };
        self.finished.store(true, Ordering::Release);
        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        let error = self
            .stderr
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        Ok((status, String::from_utf8_lossy(&error).trim().to_string()))
    }
    pub fn wait_success(&mut self, context: &str) -> Result<(), String> {
        let (status, error) = self.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{context} failed ({status}): {error}"))
        }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.stdin.take();
        self.stdout.take();
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.finished.store(true, Ordering::Release);
        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.join();
        }
    }
}

/// Publishing via a hard link is atomic and fails when the destination exists.
/// Both paths are in the output directory, hence on the same filesystem.
pub fn publish_output(
    temporary: &std::path::Path,
    directory: &std::path::Path,
    name: &str,
    extension: &str,
) -> Result<std::path::PathBuf, String> {
    for suffix in 0..10000 {
        let stem = if suffix == 0 {
            name.into()
        } else {
            format!("{name}-{suffix}")
        };
        let output = directory.join(format!("{stem}.{extension}"));
        match std::fs::hard_link(temporary, &output) {
            Ok(()) => {
                std::fs::remove_file(temporary).map_err(|e| e.to_string())?;
                return Ok(output);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot publish completed output: {e}")),
        }
    }
    Err("Cannot reserve an output name".into())
}
pub struct TemporaryOutput(pub std::path::PathBuf);
impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
