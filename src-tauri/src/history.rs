use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use crate::audio::AudioMixConfig;
use crate::export::ExportSettings;
use crate::plan::ProjectPlan;
use crate::presets::RemapParams;

const MAX_HISTORY_CAPACITY: usize = 50;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub plan: Option<ProjectPlan>,
    pub remap_params: Option<RemapParams>,
    pub export_settings: Option<ExportSettings>,
    pub audio_mix: Option<AudioMixConfig>,
    pub description: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_count: usize,
    pub redo_count: usize,
}

pub struct HistoryManager {
    pub undo_stack: VecDeque<HistorySnapshot>,
    pub redo_stack: VecDeque<HistorySnapshot>,
    pub current_state: Option<HistorySnapshot>,
    pub max_capacity: usize,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(MAX_HISTORY_CAPACITY),
            redo_stack: VecDeque::with_capacity(MAX_HISTORY_CAPACITY),
            current_state: None,
            max_capacity: MAX_HISTORY_CAPACITY,
        }
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(capacity),
            redo_stack: VecDeque::with_capacity(capacity),
            current_state: None,
            max_capacity: capacity,
        }
    }

    pub fn push_state(&mut self, snapshot: HistorySnapshot) -> HistoryStatus {
        if let Some(prev) = self.current_state.take() {
            if self.undo_stack.len() >= self.max_capacity {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(prev);
        }
        self.current_state = Some(snapshot);
        self.redo_stack.clear();
        self.get_status()
    }

    pub fn undo(&mut self) -> Option<HistorySnapshot> {
        let prev_state = self.undo_stack.pop_back()?;
        if let Some(curr) = self.current_state.take() {
            self.redo_stack.push_back(curr);
        }
        self.current_state = Some(prev_state.clone());
        Some(prev_state)
    }

    pub fn redo(&mut self) -> Option<HistorySnapshot> {
        let next_state = self.redo_stack.pop_back()?;
        if let Some(curr) = self.current_state.take() {
            if self.undo_stack.len() >= self.max_capacity {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(curr);
        }
        self.current_state = Some(next_state.clone());
        Some(next_state)
    }

    pub fn get_status(&self) -> HistoryStatus {
        HistoryStatus {
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            undo_count: self.undo_stack.len(),
            redo_count: self.redo_stack.len(),
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_state = None;
    }
}

static GLOBAL_HISTORY_MANAGER: std::sync::OnceLock<Arc<Mutex<HistoryManager>>> = std::sync::OnceLock::new();

pub fn global_history_manager() -> &'static Arc<Mutex<HistoryManager>> {
    GLOBAL_HISTORY_MANAGER.get_or_init(|| Arc::new(Mutex::new(HistoryManager::default())))
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn push_history_state(snapshot: HistorySnapshot) -> Result<HistoryStatus, String> {
    let mgr = global_history_manager();
    let mut lock = mgr.lock().map_err(|e| e.to_string())?;
    Ok(lock.push_state(snapshot))
}

#[tauri::command]
pub fn undo() -> Result<Option<HistorySnapshot>, String> {
    let mgr = global_history_manager();
    let mut lock = mgr.lock().map_err(|e| e.to_string())?;
    Ok(lock.undo())
}

#[tauri::command]
pub fn redo() -> Result<Option<HistorySnapshot>, String> {
    let mgr = global_history_manager();
    let mut lock = mgr.lock().map_err(|e| e.to_string())?;
    Ok(lock.redo())
}

#[tauri::command]
pub fn get_history_status() -> Result<HistoryStatus, String> {
    let mgr = global_history_manager();
    let lock = mgr.lock().map_err(|e| e.to_string())?;
    Ok(lock.get_status())
}
