use crate::model::{Sample, VramProcessSample};
use crate::nvml::NvmlWrapper;
use crate::targets::TargetManager;
use std::time::Instant;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub struct Sampler {
    system: System,
    start_time: Instant,
    num_cpus: usize,
}

impl Sampler {
    pub fn new() -> Self {
        let system = System::new();
        let num_cpus = system.cpus().len().max(1);
        Self {
            system,
            start_time: Instant::now(),
            num_cpus,
        }
    }

    pub fn ts_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn sample_targets(
        &mut self,
        targets: &mut TargetManager,
        nvml: Option<&NvmlWrapper>,
    ) -> (Vec<Sample>, Vec<VramProcessSample>) {
        let pids: Vec<u32> = targets.alive_pids();
        let ts_ms = self.ts_ms();

        let pid_list: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();

        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pid_list),
            false,
            ProcessRefreshKind::new().with_memory().with_cpu(),
        );

        let mut ram_samples = Vec::new();
        let mut vram_samples = Vec::new();

        for &pid in &pids {
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
                targets.mark_dead(pid);
            }
        }

        (ram_samples, vram_samples)
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}
