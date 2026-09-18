use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddMode {
    Pid,
    Exact,
    Substring,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub starttime: u64,
    pub alive: bool,
    pub add_mode: AddMode,
    pub added_at: Instant,
}

impl Target {
    pub fn new(pid: u32, name: String, cmdline: String, starttime: u64, add_mode: AddMode) -> Self {
        Self {
            pid,
            name,
            cmdline,
            starttime,
            alive: true,
            add_mode,
            added_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sample {
    pub ts_ms: u64,
    pub pid: u32,
    pub rss_bytes: u64,
    pub cpu_pct: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VramBoardSample {
    pub ts_ms: u64,
    pub gpu_index: u32,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VramProcessSample {
    pub ts_ms: u64,
    pub pid: u32,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowConfig {
    pub window_secs: u32,
    pub interval_ms: u32,
}

impl WindowConfig {
    pub const VALID_WINDOWS: [u32; 4] = [30, 300, 1800, 3600];
    pub const VALID_INTERVALS: [u32; 5] = [100, 250, 500, 1000, 2000];
    pub const MAX_POINTS: usize = 1200;

    pub fn new(window_secs: u32, interval_ms: u32) -> Option<Self> {
        if Self::VALID_WINDOWS.contains(&window_secs)
            && Self::VALID_INTERVALS.contains(&interval_ms)
        {
            Some(Self {
                window_secs,
                interval_ms,
            })
        } else {
            None
        }
    }

    pub fn bucket_ms(&self) -> u32 {
        let min_bucket = (self.window_secs * 1000) / Self::MAX_POINTS as u32;
        self.interval_ms.max(min_bucket)
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_secs: 30,
            interval_ms: 1000,
        }
    }
}
