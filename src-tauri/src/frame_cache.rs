//! Decode only requested two-second blocks. Reverse and loops reuse blocks;
//! eviction bounds both the raw files and the in-memory frame LRU.
use crate::{process::ManagedChild, sources::SourceRange};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub const DEFAULT_DISK_BUDGET: u64 = 4 * 1024 * 1024 * 1024;
pub const DEFAULT_RAM_BUDGET: usize = 256 * 1024 * 1024;
struct IndexedRange {
    range: SourceRange,
    compact_start: f64,
    first_frame: u64,
    frames: u64,
}
struct Block {
    path: PathBuf,
    frames: u64,
    bytes: u64,
}
pub struct FrameCache {
    ffmpeg: PathBuf,
    paths: Vec<String>,
    ranges: Vec<IndexedRange>,
    directory: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
    frame_bytes: usize,
    block_frames: u64,
    cancelled: Arc<AtomicBool>,
    blocks: HashMap<(usize, u64), Block>,
    order: VecDeque<(usize, u64)>,
    ram: HashMap<u64, Vec<u8>>,
    ram_order: VecDeque<u64>,
    ram_capacity: usize,
    disk_budget: u64,
    disk_bytes: u64,
    pub decoded_bytes: u64,
    pub peak_disk_bytes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_ram_bytes: u64,
    pub decode_time: Duration,
}
impl FrameCache {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ffmpeg: PathBuf,
        paths: Vec<String>,
        ranges: Vec<SourceRange>,
        directory: PathBuf,
        width: u32,
        height: u32,
        fps: f64,
        cancelled: Arc<AtomicBool>,
        ram_budget: usize,
        disk_budget: u64,
    ) -> Result<Self, String> {
        let frame_bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(3))
            .ok_or("Frame allocation overflow")?;
        if frame_bytes == 0
            || disk_budget < frame_bytes as u64
            || !fps.is_finite()
            || fps <= 0.0
            || fps > 120.0
            || ranges.is_empty()
            || ranges.iter().any(|range| {
                range.source_id >= paths.len()
                    || !range.start.is_finite()
                    || range.start < 0.0
                    || !range.duration.is_finite()
                    || range.duration <= 0.0
            })
        {
            return Err("Invalid frame cache configuration".into());
        }
        let mut first_frame = 0;
        let mut compact_start = 0.0;
        let ranges = ranges
            .into_iter()
            .map(|range| {
                let frames = (range.duration * fps).ceil().max(1.0) as u64;
                let indexed = IndexedRange {
                    range,
                    compact_start,
                    first_frame,
                    frames,
                };
                first_frame += frames;
                compact_start += indexed.range.duration;
                indexed
            })
            .collect();
        // A block is at most two seconds and always fits the disk budget; it streams to disk.
        let block_frames = ((fps * 2.0).ceil() as u64)
            .min(disk_budget / frame_bytes as u64)
            .max(1);
        Ok(Self {
            ffmpeg,
            paths,
            ranges,
            directory,
            width,
            height,
            fps,
            frame_bytes,
            block_frames,
            cancelled,
            blocks: HashMap::new(),
            order: VecDeque::new(),
            ram: HashMap::new(),
            ram_order: VecDeque::new(),
            ram_capacity: (ram_budget / frame_bytes).min(16),
            disk_budget,
            disk_bytes: 0,
            decoded_bytes: 0,
            peak_disk_bytes: 0,
            cache_hits: 0,
            cache_misses: 0,
            peak_ram_bytes: 0,
            decode_time: Duration::ZERO,
        })
    }
    pub fn total_frames(&self) -> u64 {
        self.ranges.iter().map(|r| r.frames).sum()
    }
    pub fn frame_at_time(&self, time: f64) -> (u64, u64, u64) {
        let index = self
            .ranges
            .iter()
            .rposition(|r| time >= r.compact_start)
            .unwrap_or(0);
        let range = &self.ranges[index];
        let frame = ((time - range.compact_start).max(0.0) * self.fps).round() as u64;
        (
            range.first_frame + frame.min(range.frames - 1),
            range.first_frame,
            range.first_frame + range.frames - 1,
        )
    }
    fn check_cancelled(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Acquire) {
            Err("Render cancelled by user".into())
        } else {
            Ok(())
        }
    }
    pub fn get_frame(&mut self, frame: u64, output: &mut [u8]) -> Result<(), String> {
        self.check_cancelled()?;
        if output.len() != self.frame_bytes {
            return Err("Invalid frame buffer size".into());
        }
        if let Some(cached) = self.ram.get(&frame) {
            output.copy_from_slice(cached);
            self.ram_order.retain(|item| *item != frame);
            self.ram_order.push_back(frame);
            self.cache_hits += 1;
            return Ok(());
        }
        self.cache_misses += 1;
        let index = self
            .ranges
            .iter()
            .position(|r| frame >= r.first_frame && frame < r.first_frame + r.frames)
            .ok_or("Frame outside selected ranges")?;
        let local = frame - self.ranges[index].first_frame;
        let block_start = local / self.block_frames * self.block_frames;
        let key = (index, block_start);
        if !self.blocks.contains_key(&key) {
            self.decode_block(index, block_start)?;
        }
        self.order.retain(|item| *item != key);
        self.order.push_back(key);
        let block = &self.blocks[&key];
        let offset = (local - block_start).min(block.frames - 1) * self.frame_bytes as u64;
        let mut file = std::fs::File::open(&block.path).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(output))
            .map_err(|e| format!("Cannot read cached frame: {e}"))?;
        if self.ram_capacity > 0 {
            if self.ram.len() >= self.ram_capacity {
                if let Some(old) = self.ram_order.pop_front() {
                    self.ram.remove(&old);
                }
            }
            self.ram.insert(frame, output.to_vec());
            self.ram_order.push_back(frame);
            self.peak_ram_bytes = self
                .peak_ram_bytes
                .max((self.ram.len() * self.frame_bytes) as u64);
        }
        Ok(())
    }
    fn decode_block(&mut self, index: usize, block_start: u64) -> Result<(), String> {
        let started = Instant::now();
        let range = &self.ranges[index];
        let frames = self.block_frames.min(range.frames - block_start);
        let reserve = frames * self.frame_bytes as u64;
        while self.disk_bytes + reserve > self.disk_budget {
            let oldest = self
                .order
                .pop_front()
                .ok_or("Raw frame cache budget is too small")?;
            if let Some(block) = self.blocks.remove(&oldest) {
                std::fs::remove_file(block.path)
                    .map_err(|e| format!("Cannot evict frame cache: {e}"))?;
                self.disk_bytes -= block.bytes;
            }
        }
        ensure_disk_space(&self.directory, reserve + 16 * 1024 * 1024)?;
        let start = range.range.start + block_start as f64 / self.fps;
        let duration =
            (frames as f64 / self.fps).min(range.range.duration - block_start as f64 / self.fps);
        let path = self
            .directory
            .join(format!("source-{index}-block-{block_start}.raw"));
        let mut command = Command::new(&self.ffmpeg);
        command.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-ss", &format!("{start:.9}"), "-i", &self.paths[range.range.source_id],
            "-t", &format!("{duration:.9}"), "-vf", &format!("trim=duration={duration:.9},setpts=PTS-STARTPTS,scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={:.9}", self.width, self.height, self.width, self.height, self.fps),
            "-frames:v", &frames.to_string(), "-f", "rawvideo", "-pix_fmt", "rgb24", "-an", "pipe:1"]);
        command.stdout(Stdio::piped());
        let mut child = ManagedChild::spawn(&mut command, self.cancelled.clone())?;
        let mut stdout = child.stdout.take().ok_or("Decoder stdout missing")?;
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; 1024 * 1024];
        let mut bytes = 0u64;
        loop {
            self.check_cancelled()?;
            let count = stdout
                .read(&mut buffer)
                .map_err(|e| format!("Decoder pipe failed: {e}"))?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > reserve {
                return Err("Decoder exceeded the reserved cache block".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|e| format!("Frame cache write failed: {e}"))?;
        }
        file.flush().map_err(|e| e.to_string())?;
        child.wait_success(&format!("Decoding source {}", range.range.source_id + 1))?;
        self.check_cancelled()?;
        if bytes == 0 || !bytes.is_multiple_of(self.frame_bytes as u64) {
            return Err(format!(
                "Source {} produced no complete frames",
                range.range.source_id + 1
            ));
        }
        self.disk_bytes += bytes;
        self.decoded_bytes += bytes;
        self.peak_disk_bytes = self.peak_disk_bytes.max(self.disk_bytes);
        self.decode_time += started.elapsed();
        self.blocks.insert(
            (index, block_start),
            Block {
                path,
                frames: bytes / self.frame_bytes as u64,
                bytes,
            },
        );
        self.order.push_back((index, block_start));
        Ok(())
    }
}

#[cfg(windows)]
pub fn ensure_disk_space(directory: &Path, required: u64) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            path: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0;
    // Windows writes one u64 into the provided live buffer; optional outputs are null.
    if unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(format!(
            "Cannot check free disk space: {}",
            std::io::Error::last_os_error()
        ));
    }
    if available < required {
        Err(format!(
            "Insufficient free disk space: need {required} bytes, have {available}"
        ))
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
pub fn ensure_disk_space(_directory: &Path, _required: u64) -> Result<(), String> {
    Ok(())
}
