use crate::model::{Sample, VramProcessSample};
use crate::nvml::NvmlWrapper;
use std::time::Instant;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub struct Sampler {
    system: System,
    start_time: Instant,
    num_cpus: usize,
    num_cpus_detected: bool,
}

impl Sampler {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_all();
        let cpu_count = system.cpus().len();
        let num_cpus_detected = cpu_count > 0;
        let num_cpus = if num_cpus_detected { cpu_count } else {
            eprintln!("WARNING: Could not detect CPU count, defaulting to 1. CPU% may be inaccurate.");
            1
        };
        Self {
            system,
            start_time: Instant::now(),
            num_cpus,
            num_cpus_detected,
        }
    }

    pub fn ts_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn num_cpus(&self) -> usize {
        self.num_cpus
    }

    pub fn num_cpus_detected(&self) -> bool {
        self.num_cpus_detected
    }

    pub fn sample_pids(
        &mut self,
        pids: &[u32],
        nvml: Option<&NvmlWrapper>,
    ) -> SampleResult {
        let ts_ms = self.ts_ms();

        let pid_list: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();

        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pid_list),
            false,
            ProcessRefreshKind::new().with_memory().with_cpu(),
        );

        let mut ram_samples = Vec::new();
        let mut vram_samples = Vec::new();
        let mut not_found = Vec::new();

        for &pid in pids {
            let sysinfo_pid = Pid::from_u32(pid);

            if let Some(process) = self.system.process(sysinfo_pid) {
                let rss_bytes = process.memory();

                let cpu_time = process.cpu_usage();
                let cpu_pct = cpu_time / self.num_cpus as f32;

                ram_samples.push(Sample {
                    ts_ms,
                    pid,
                    rss_bytes,
                    cpu_pct,
                });

                if let Some(nvml) = nvml {
                    if let Some(used_bytes) = nvml.get_process_vram(pid) {
                        vram_samples.push(VramProcessSample {
                            ts_ms,
                            pid,
                            used_bytes,
                        });
                    }
                }
            } else {
                not_found.push(pid);
            }
        }

        SampleResult {
            ram_samples,
            vram_samples,
            not_found,
        }
    }
}

pub struct SampleResult {
    pub ram_samples: Vec<Sample>,
    pub vram_samples: Vec<VramProcessSample>,
    pub not_found: Vec<u32>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}
