//! Benchmark instrumentation: sample the render process, excluding FFmpeg children.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(windows)]
fn working_set() -> u64 {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_nonpaged_pool_usage: usize,
        quota_nonpaged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    let mut counters = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Counters::default()
    };
    // Pseudo-handle remains valid and the stack buffer matches PROCESS_MEMORY_COUNTERS.
    if unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) } == 0 {
        0
    } else {
        counters.working_set_size as u64
    }
}
#[cfg(not(windows))]
fn working_set() -> u64 {
    0
}

pub fn measure<T>(render: impl FnOnce() -> Result<T, String>) -> Result<(T, u64), String> {
    let finished = Arc::new(AtomicBool::new(false));
    let flag = finished.clone();
    let monitor = std::thread::spawn(move || {
        let mut peak = working_set();
        while !flag.load(Ordering::Acquire) {
            peak = peak.max(working_set());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        peak.max(working_set())
    });
    let result = render();
    finished.store(true, Ordering::Release);
    let peak = monitor.join().map_err(|_| "Memory sampler failed")?;
    result.map(|value| (value, peak))
}
