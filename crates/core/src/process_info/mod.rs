#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("Beholder supports Linux and Windows only");

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(super) use linux::{find_processes_by_name, get_starttime, read_process_info};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(super) use windows::{find_processes_by_name, get_starttime, read_process_info};

pub(super) struct FindResult {
    pub matches: Vec<(u32, String, String, u64)>,
    pub permission_denied_count: usize,
}
