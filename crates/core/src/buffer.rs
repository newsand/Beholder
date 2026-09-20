use crate::model::{Sample, VramProcessSample, WindowConfig};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
struct Bucket {
    rss_ts_ms: u64,
    rss_bytes: u64,
    cpu_ts_ms: u64,
    cpu_pct: f32,
}

#[derive(Debug)]
pub struct RingSeries {
    downsampled: VecDeque<Bucket>,
    current_bucket_start: Option<u64>,
    current_bucket_max_rss: Option<Sample>,
    current_bucket_last_cpu: Option<Sample>,
    last_raw_sample: Option<Sample>,
    gauge_min_rss: Option<u64>,
    gauge_max_rss: Option<u64>,
}

impl RingSeries {
    pub fn new() -> Self {
        Self {
            downsampled: VecDeque::new(),
            current_bucket_start: None,
            current_bucket_max_rss: None,
            current_bucket_last_cpu: None,
            last_raw_sample: None,
            gauge_min_rss: None,
            gauge_max_rss: None,
        }
    }

    /// Record a sample. `last_raw_sample` and the gauge min/max/current are
    /// always kept up to date (O(1)). When `track_history` is false, no
    /// points are appended to the downsampled history, so memory stays
    /// O(1) per target instead of growing with the window.
    pub fn push(&mut self, sample: Sample, config: &WindowConfig, track_history: bool) {
        self.last_raw_sample = Some(sample);
        self.gauge_min_rss = Some(self.gauge_min_rss.map_or(sample.rss_bytes, |m| m.min(sample.rss_bytes)));
        self.gauge_max_rss = Some(self.gauge_max_rss.map_or(sample.rss_bytes, |m| m.max(sample.rss_bytes)));

        if !track_history {
            return;
        }

        let bucket_ms = config.bucket_ms() as u64;
        let window_ms = config.window_secs as u64 * 1000;

        let bucket_start = (sample.ts_ms / bucket_ms) * bucket_ms;

        match self.current_bucket_start {
            Some(start) if start == bucket_start => {
                if let Some(ref max) = self.current_bucket_max_rss {
                    if sample.rss_bytes > max.rss_bytes {
                        self.current_bucket_max_rss = Some(sample);
                    }
                } else {
                    self.current_bucket_max_rss = Some(sample);
                }
                self.current_bucket_last_cpu = Some(sample);
            }
            Some(_) => {
                self.flush_bucket();
                self.current_bucket_start = Some(bucket_start);
                self.current_bucket_max_rss = Some(sample);
                self.current_bucket_last_cpu = Some(sample);
            }
            None => {
                self.current_bucket_start = Some(bucket_start);
                self.current_bucket_max_rss = Some(sample);
                self.current_bucket_last_cpu = Some(sample);
            }
        }

        let cutoff = sample.ts_ms.saturating_sub(window_ms);
        while let Some(front) = self.downsampled.front() {
            if front.rss_ts_ms < cutoff {
                self.downsampled.pop_front();
            } else {
                break;
            }
        }

        while self.downsampled.len() > WindowConfig::MAX_POINTS {
            self.downsampled.pop_front();
        }
    }

    /// Current value + running min/max, as tracked in gauge mode (also kept
    /// up to date in history mode).
    pub fn gauge(&self) -> Option<(u64, u64, u64)> {
        Some((
            self.last_raw_sample?.rss_bytes,
            self.gauge_min_rss?,
            self.gauge_max_rss?,
        ))
    }

    pub fn reset_gauge(&mut self) {
        self.gauge_min_rss = None;
        self.gauge_max_rss = None;
    }

    fn flush_bucket(&mut self) {
        if let (Some(max_sample), Some(cpu_sample)) =
            (self.current_bucket_max_rss.take(), self.current_bucket_last_cpu.take())
        {
            self.downsampled.push_back(Bucket {
                rss_ts_ms: max_sample.ts_ms,
                rss_bytes: max_sample.rss_bytes,
                cpu_ts_ms: cpu_sample.ts_ms,
                cpu_pct: cpu_sample.cpu_pct,
            });
        }
    }

    pub fn get_points(&self) -> Vec<(u64, u64, u64, f32)> {
        let mut points: Vec<_> = self
            .downsampled
            .iter()
            .map(|b| (b.rss_ts_ms, b.rss_bytes, b.cpu_ts_ms, b.cpu_pct))
            .collect();

        if let (Some(max), Some(cpu)) = (&self.current_bucket_max_rss, &self.current_bucket_last_cpu)
        {
            points.push((max.ts_ms, max.rss_bytes, cpu.ts_ms, cpu.cpu_pct));
        }

        points
    }

    pub fn peak_rss(&self) -> Option<(u64, u64)> {
        let mut max: Option<(u64, u64)> = None;

        for b in &self.downsampled {
            match max {
                Some((_, rss)) if b.rss_bytes > rss => max = Some((b.rss_ts_ms, b.rss_bytes)),
                None => max = Some((b.rss_ts_ms, b.rss_bytes)),
                _ => {}
            }
        }

        if let Some(ref s) = self.current_bucket_max_rss {
            match max {
                Some((_, rss)) if s.rss_bytes > rss => max = Some((s.ts_ms, s.rss_bytes)),
                None => max = Some((s.ts_ms, s.rss_bytes)),
                _ => {}
            }
        }

        max
    }

    pub fn latest_raw(&self) -> Option<&Sample> {
        self.last_raw_sample.as_ref()
    }

    pub fn latest_downsampled(&self) -> Option<(u64, u64, u64, f32)> {
        if let (Some(max), Some(cpu)) = (&self.current_bucket_max_rss, &self.current_bucket_last_cpu)
        {
            return Some((max.ts_ms, max.rss_bytes, cpu.ts_ms, cpu.cpu_pct));
        }
        self.downsampled
            .back()
            .map(|b| (b.rss_ts_ms, b.rss_bytes, b.cpu_ts_ms, b.cpu_pct))
    }

    pub fn count(&self) -> usize {
        self.downsampled.len() + if self.current_bucket_max_rss.is_some() { 1 } else { 0 }
    }

    pub fn clear(&mut self) {
        self.downsampled.clear();
        self.current_bucket_start = None;
        self.current_bucket_max_rss = None;
        self.current_bucket_last_cpu = None;
        self.last_raw_sample = None;
        self.reset_gauge();
    }
}

impl Default for RingSeries {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct VramBucket {
    ts_ms: u64,
    used_bytes: u64,
}

#[derive(Debug)]
pub struct VramRingSeries {
    downsampled: VecDeque<VramBucket>,
    current_bucket_start: Option<u64>,
    current_bucket_max: Option<VramProcessSample>,
    last_raw_sample: Option<VramProcessSample>,
    gauge_min: Option<u64>,
    gauge_max: Option<u64>,
}

impl VramRingSeries {
    pub fn new() -> Self {
        Self {
            downsampled: VecDeque::new(),
            current_bucket_start: None,
            current_bucket_max: None,
            last_raw_sample: None,
            gauge_min: None,
            gauge_max: None,
        }
    }

    /// Record a sample. `last_raw_sample` and the gauge min/max are always
    /// kept up to date. When `track_history` is false, no points are
    /// appended to the downsampled history.
    pub fn push(&mut self, sample: VramProcessSample, config: &WindowConfig, track_history: bool) {
        self.last_raw_sample = Some(sample);
        self.gauge_min = Some(self.gauge_min.map_or(sample.used_bytes, |m| m.min(sample.used_bytes)));
        self.gauge_max = Some(self.gauge_max.map_or(sample.used_bytes, |m| m.max(sample.used_bytes)));

        if !track_history {
            return;
        }

        let bucket_ms = config.bucket_ms() as u64;
        let window_ms = config.window_secs as u64 * 1000;

        let bucket_start = (sample.ts_ms / bucket_ms) * bucket_ms;

        match self.current_bucket_start {
            Some(start) if start == bucket_start => {
                if let Some(ref max) = self.current_bucket_max {
                    if sample.used_bytes > max.used_bytes {
                        self.current_bucket_max = Some(sample);
                    }
                } else {
                    self.current_bucket_max = Some(sample);
                }
            }
            Some(_) => {
                self.flush_bucket();
                self.current_bucket_start = Some(bucket_start);
                self.current_bucket_max = Some(sample);
            }
            None => {
                self.current_bucket_start = Some(bucket_start);
                self.current_bucket_max = Some(sample);
            }
        }

        let cutoff = sample.ts_ms.saturating_sub(window_ms);
        while let Some(front) = self.downsampled.front() {
            if front.ts_ms < cutoff {
                self.downsampled.pop_front();
            } else {
                break;
            }
        }

        while self.downsampled.len() > WindowConfig::MAX_POINTS {
            self.downsampled.pop_front();
        }
    }

    fn flush_bucket(&mut self) {
        if let Some(max_sample) = self.current_bucket_max.take() {
            self.downsampled.push_back(VramBucket {
                ts_ms: max_sample.ts_ms,
                used_bytes: max_sample.used_bytes,
            });
        }
    }

    pub fn get_points(&self) -> Vec<(u64, u64)> {
        let mut points: Vec<_> = self
            .downsampled
            .iter()
            .map(|b| (b.ts_ms, b.used_bytes))
            .collect();

        if let Some(ref max) = self.current_bucket_max {
            points.push((max.ts_ms, max.used_bytes));
        }

        points
    }

    pub fn peak(&self) -> Option<(u64, u64)> {
        let mut max: Option<(u64, u64)> = None;

        for b in &self.downsampled {
            match max {
                Some((_, used)) if b.used_bytes > used => max = Some((b.ts_ms, b.used_bytes)),
                None => max = Some((b.ts_ms, b.used_bytes)),
                _ => {}
            }
        }

        if let Some(ref s) = self.current_bucket_max {
            match max {
                Some((_, used)) if s.used_bytes > used => max = Some((s.ts_ms, s.used_bytes)),
                None => max = Some((s.ts_ms, s.used_bytes)),
                _ => {}
            }
        }

        max
    }

    pub fn latest_raw(&self) -> Option<&VramProcessSample> {
        self.last_raw_sample.as_ref()
    }

    pub fn latest_downsampled(&self) -> Option<(u64, u64)> {
        if let Some(ref max) = self.current_bucket_max {
            return Some((max.ts_ms, max.used_bytes));
        }
        self.downsampled.back().map(|b| (b.ts_ms, b.used_bytes))
    }

    pub fn count(&self) -> usize {
        self.downsampled.len() + if self.current_bucket_max.is_some() { 1 } else { 0 }
    }

    /// Current value + running min/max, as tracked in gauge mode (also kept
    /// up to date in history mode).
    pub fn gauge(&self) -> Option<(u64, u64, u64)> {
        Some((
            self.last_raw_sample?.used_bytes,
            self.gauge_min?,
            self.gauge_max?,
        ))
    }

    pub fn reset_gauge(&mut self) {
        self.gauge_min = None;
        self.gauge_max = None;
    }

    pub fn clear(&mut self) {
        self.downsampled.clear();
        self.current_bucket_start = None;
        self.current_bucket_max = None;
        self.last_raw_sample = None;
        self.reset_gauge();
    }
}

impl Default for VramRingSeries {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct BufferManager {
    ram_series: HashMap<u32, RingSeries>,
    vram_series: HashMap<u32, VramRingSeries>,
    config: WindowConfig,
}

impl BufferManager {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            ram_series: HashMap::new(),
            vram_series: HashMap::new(),
            config,
        }
    }

    pub fn set_config(&mut self, config: WindowConfig) {
        self.config = config;
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }

    pub fn push_ram(&mut self, sample: Sample, track_history: bool) {
        let series = self.ram_series.entry(sample.pid).or_default();
        series.push(sample, &self.config, track_history);
    }

    pub fn push_vram(&mut self, sample: VramProcessSample, track_history: bool) {
        let series = self.vram_series.entry(sample.pid).or_default();
        series.push(sample, &self.config, track_history);
    }

    pub fn get_ram_series(&self, pid: u32) -> Option<&RingSeries> {
        self.ram_series.get(&pid)
    }

    pub fn get_vram_series(&self, pid: u32) -> Option<&VramRingSeries> {
        self.vram_series.get(&pid)
    }

    pub fn all_ram_series(&self) -> &HashMap<u32, RingSeries> {
        &self.ram_series
    }

    pub fn all_vram_series(&self) -> &HashMap<u32, VramRingSeries> {
        &self.vram_series
    }

    pub fn clear(&mut self) {
        for series in self.ram_series.values_mut() {
            series.clear();
        }
        for series in self.vram_series.values_mut() {
            series.clear();
        }
    }

    pub fn remove_target(&mut self, pid: u32) {
        self.ram_series.remove(&pid);
        self.vram_series.remove(&pid);
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new(WindowConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample(ts_ms: u64, pid: u32, rss_bytes: u64, cpu_pct: f32) -> Sample {
        Sample {
            ts_ms,
            pid,
            rss_bytes,
            cpu_pct,
        }
    }

    #[test]
    fn test_downsample_max_rss() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(100, 1, 200, 20.0), &config, true);
        series.push(make_sample(200, 1, 150, 15.0), &config, true);

        let points = series.get_points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].1, 200);
    }

    #[test]
    fn test_downsample_last_cpu() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(100, 1, 200, 20.0), &config, true);
        series.push(make_sample(200, 1, 150, 99.0), &config, true);

        let points = series.get_points();
        assert_eq!(points.len(), 1);
        assert!((points[0].3 - 99.0).abs() < 0.01);
    }

    #[test]
    fn test_cpu_timestamp_separate_from_rss() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(100, 1, 500, 10.0), &config, true);
        series.push(make_sample(200, 1, 300, 20.0), &config, true);
        series.push(make_sample(300, 1, 400, 30.0), &config, true);

        let points = series.get_points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].0, 100);
        assert_eq!(points[0].1, 500);
        assert_eq!(points[0].2, 300);
        assert!((points[0].3 - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_bucket_boundary() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(500, 1, 200, 20.0), &config, true);
        series.push(make_sample(1000, 1, 150, 15.0), &config, true);
        series.push(make_sample(1500, 1, 180, 18.0), &config, true);

        let points = series.get_points();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].1, 200);
        assert_eq!(points[1].1, 180);
    }

    #[test]
    fn test_window_eviction() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        for i in 0..40 {
            let ts = i * 1000;
            series.push(make_sample(ts, 1, 100 + i, 10.0), &config, true);
        }

        let points = series.get_points();
        assert!(points.len() <= 31);
        assert!(points[0].0 >= 9000);
    }

    #[test]
    fn test_max_points_cap() {
        let config = WindowConfig::new(3600, 100).unwrap();
        let mut series = RingSeries::new();

        for i in 0..5000 {
            let ts = i * 100;
            series.push(make_sample(ts, 1, 100, 10.0), &config, true);
        }

        assert!(series.count() <= WindowConfig::MAX_POINTS + 1);
    }

    #[test]
    fn test_peak_rss() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(1000, 1, 500, 20.0), &config, true);
        series.push(make_sample(2000, 1, 300, 15.0), &config, true);

        let peak = series.peak_rss();
        assert_eq!(peak, Some((1000, 500)));
    }

    #[test]
    fn test_clear() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(1000, 1, 200, 20.0), &config, true);

        series.clear();
        assert_eq!(series.count(), 0);
        assert!(series.peak_rss().is_none());
        assert!(series.latest_raw().is_none());
    }

    #[test]
    fn test_latest_raw_vs_downsampled() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, true);
        series.push(make_sample(100, 1, 500, 20.0), &config, true);
        series.push(make_sample(200, 1, 300, 30.0), &config, true);

        let raw = series.latest_raw().unwrap();
        assert_eq!(raw.ts_ms, 200);
        assert_eq!(raw.rss_bytes, 300);
        assert!((raw.cpu_pct - 30.0).abs() < 0.01);

        let downsampled = series.latest_downsampled().unwrap();
        assert_eq!(downsampled.1, 500);
    }

    #[test]
    fn test_gauge_tracks_min_max_current_without_history() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = RingSeries::new();

        series.push(make_sample(0, 1, 100, 10.0), &config, false);
        series.push(make_sample(1000, 1, 500, 20.0), &config, false);
        series.push(make_sample(2000, 1, 300, 15.0), &config, false);

        assert_eq!(series.count(), 0);
        assert_eq!(series.gauge(), Some((300, 100, 500)));
    }

    #[test]
    fn test_vram_downsample_max() {
        let config = WindowConfig::new(30, 1000).unwrap();
        let mut series = VramRingSeries::new();

        series.push(VramProcessSample { ts_ms: 0, pid: 1, used_bytes: 100 }, &config, true);
        series.push(VramProcessSample { ts_ms: 100, pid: 1, used_bytes: 500 }, &config, true);
        series.push(VramProcessSample { ts_ms: 200, pid: 1, used_bytes: 300 }, &config, true);

        let points = series.get_points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].1, 500);

        let raw = series.latest_raw().unwrap();
        assert_eq!(raw.used_bytes, 300);
    }
}
