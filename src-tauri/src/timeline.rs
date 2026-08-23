use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use crate::effects::evaluate_curve_derivative;
use crate::plan::ProjectPlan;
use crate::render::compute_source_time_for_target_time;

/// LRU Cache for decoded video frames
#[derive(Debug, Clone)]
pub struct LruFrameCache {
    pub capacity: usize,
    pub cache: HashMap<u64, Vec<u8>>,
    pub order: VecDeque<u64>,
    pub decode_count: usize,
}

impl Default for LruFrameCache {
    fn default() -> Self {
        Self {
            capacity: 30,
            cache: HashMap::with_capacity(30),
            order: VecDeque::with_capacity(30),
            decode_count: 0,
        }
    }
}

impl LruFrameCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            decode_count: 0,
        }
    }

    pub fn get(&mut self, frame_idx: u64) -> Option<Vec<u8>> {
        if let Some(frame) = self.cache.get(&frame_idx) {
            // Move to back of LRU order
            if let Some(pos) = self.order.iter().position(|&x| x == frame_idx) {
                self.order.remove(pos);
            }
            self.order.push_back(frame_idx);
            Some(frame.clone())
        } else {
            None
        }
    }

    pub fn insert(&mut self, frame_idx: u64, frame_data: Vec<u8>) {
        if self.cache.contains_key(&frame_idx) {
            self.cache.insert(frame_idx, frame_data);
            if let Some(pos) = self.order.iter().position(|&x| x == frame_idx) {
                self.order.remove(pos);
            }
            self.order.push_back(frame_idx);
        } else {
            if self.cache.len() >= self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.cache.remove(&oldest);
                }
            }
            self.cache.insert(frame_idx, frame_data);
            self.order.push_back(frame_idx);
        }
    }
}

// Global static scrubbing cache
static GLOBAL_SCRUB_CACHE: std::sync::OnceLock<Arc<Mutex<LruFrameCache>>> = std::sync::OnceLock::new();

pub fn global_scrub_cache() -> &'static Arc<Mutex<LruFrameCache>> {
    GLOBAL_SCRUB_CACHE.get_or_init(|| Arc::new(Mutex::new(LruFrameCache::new(30))))
}

/// Compute instantaneous velocity v(t) = ds/dt for a given target timestamp t
pub fn compute_instantaneous_velocity(plan: &ProjectPlan, t: f64) -> f64 {
    if plan.segments.is_empty() {
        return 1.0;
    }

    let clamped_t = t.clamp(0.0, plan.target_duration);

    // Find segment
    let mut matching_idx = 0;
    for (i, seg) in plan.segments.iter().enumerate() {
        if clamped_t >= seg.t0 && clamped_t <= seg.t1 {
            matching_idx = i;
            break;
        }
    }

    let seg = &plan.segments[matching_idx];
    let dt = (seg.t1 - seg.t0).max(1e-6);
    let ds = seg.s1 - seg.s0;

    // Freeze: s0 == s1 -> velocity is strictly 0.0
    if ds.abs() < 1e-6 {
        return 0.0;
    }

    let u = ((clamped_t - seg.t0) / dt).clamp(0.0, 1.0);
    let d_curve = evaluate_curve_derivative(&seg.curve, u);

    // v = (ds / dt) * d_curve
    // If reverse (ds < 0), v is negative
    (ds / dt) * d_curve
}

/// Retrieve or decode a frame for scrubbing with LRU cache
pub fn get_scrub_frame_cached(
    target_time_s: f64,
    plan: &ProjectPlan,
    cache: &mut LruFrameCache,
) -> (u64, Vec<u8>) {
    let (s_time, _seg_idx) = compute_source_time_for_target_time(plan, target_time_s);
    let src_fps = if plan.fps > 0 { plan.fps as f64 } else { 30.0 };
    let frame_idx = (s_time * src_fps).round().max(0.0) as u64;

    if let Some(cached_data) = cache.get(frame_idx) {
        return (frame_idx, cached_data);
    }

    // Cache miss -> decode / synthesize 256x256 RGB frame
    cache.decode_count += 1;
    let mut synthetic = vec![0u8; 256 * 256 * 3];
    let r = ((frame_idx * 17) % 256) as u8;
    let g = ((frame_idx * 31) % 256) as u8;
    let b = ((frame_idx * 53) % 256) as u8;
    for px in synthetic.chunks_exact_mut(3) {
        px[0] = r;
        px[1] = g;
        px[2] = b;
    }

    cache.insert(frame_idx, synthetic.clone());
    (frame_idx, synthetic)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_scrub_frame(target_time_ms: f64, plan: ProjectPlan) -> Result<String, String> {
    let target_time_s = target_time_ms / 1000.0;
    let cache_lock = global_scrub_cache();
    let mut cache = cache_lock.lock().map_err(|e| format!("Lock error: {e}"))?;

    let (frame_idx, _data) = get_scrub_frame_cached(target_time_s, &plan, &mut cache);

    // Return JSON payload with frame index & source time
    let (s_time, _) = compute_source_time_for_target_time(&plan, target_time_s);
    let velocity = compute_instantaneous_velocity(&plan, target_time_s);

    Ok(format!(
        "{{\"frameIndex\":{},\"sourceTime\":{:.3},\"velocity\":{:.3}}}",
        frame_idx, s_time, velocity
    ))
}

#[tauri::command]
pub fn get_time_curve_velocities(plan: ProjectPlan, samples_count: usize) -> Result<Vec<f64>, String> {
    let count = samples_count.clamp(10, 1000);
    let duration = plan.target_duration.max(0.1);
    let mut velocities = Vec::with_capacity(count);

    for i in 0..count {
        let t = (i as f64 / (count - 1) as f64) * duration;
        let v = compute_instantaneous_velocity(&plan, t);
        velocities.push(v);
    }

    Ok(velocities)
}
