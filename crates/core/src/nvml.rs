use crate::model::VramBoardSample;
use nvml_wrapper::enums::device::UsedGpuMemory;
use nvml_wrapper::Nvml;
use std::time::Instant;

pub struct NvmlWrapper {
    nvml: Nvml,
    start_time: Instant,
}

#[derive(Debug)]
pub enum NvmlError {
    InitFailed(String),
    DeviceNotFound,
    QueryFailed(String),
}

impl NvmlWrapper {
    pub fn new() -> Result<Self, NvmlError> {
        let nvml = Nvml::init().map_err(|e| NvmlError::InitFailed(e.to_string()))?;
        Ok(Self {
            nvml,
            start_time: Instant::now(),
        })
    }

    fn ts_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn get_board_vram(&self) -> Result<VramBoardSample, NvmlError> {
        let device = self
            .nvml
            .device_by_index(0)
            .map_err(|_| NvmlError::DeviceNotFound)?;

        let memory_info = device
            .memory_info()
            .map_err(|e| NvmlError::QueryFailed(e.to_string()))?;

        Ok(VramBoardSample {
            ts_ms: self.ts_ms(),
            gpu_index: 0,
            total_bytes: memory_info.total,
            used_bytes: memory_info.used,
            free_bytes: memory_info.free,
        })
    }

    pub fn get_process_vram(&self, pid: u32) -> Option<u64> {
        let device = self.nvml.device_by_index(0).ok()?;

        let processes = device.running_compute_processes().ok()?;

        for proc in processes {
            if proc.pid == pid {
                return match proc.used_gpu_memory {
                    UsedGpuMemory::Used(bytes) => Some(bytes),
                    UsedGpuMemory::Unavailable => None,
                };
            }
        }

        let graphics = device.running_graphics_processes().ok()?;
        for proc in graphics {
            if proc.pid == pid {
                return match proc.used_gpu_memory {
                    UsedGpuMemory::Used(bytes) => Some(bytes),
                    UsedGpuMemory::Unavailable => None,
                };
            }
        }

        None
    }

    pub fn is_available(&self) -> bool {
        self.nvml.device_by_index(0).is_ok()
    }
}

pub fn try_init_nvml() -> Option<NvmlWrapper> {
    NvmlWrapper::new().ok()
}
