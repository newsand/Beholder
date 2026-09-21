use super::FindResult;
use crate::model::AddMode;
use crate::targets::TargetError;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::new()
        .with_cmd(UpdateKind::Always)
        .with_exe(UpdateKind::Always)
}

fn refresh(pids: ProcessesToUpdate<'_>) -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(pids, true, refresh_kind());
    system
}

fn image_name(process: &Process) -> String {
    process.name().to_string_lossy().into_owned()
}

fn cmdline(process: &Process) -> String {
    process
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn names_equal(proc_name: &str, query: &str) -> bool {
    if proc_name.eq_ignore_ascii_case(query) {
        return true;
    }
    let stem = proc_name
        .strip_suffix(".exe")
        .or_else(|| proc_name.strip_suffix(".EXE"))
        .unwrap_or(proc_name);
    stem.eq_ignore_ascii_case(query)
}

pub fn read_process_info(pid: u32) -> Result<(String, String, u64), TargetError> {
    let sysinfo_pid = Pid::from_u32(pid);
    let system = refresh(ProcessesToUpdate::Some(&[sysinfo_pid]));
    let process = system
        .process(sysinfo_pid)
        .ok_or(TargetError::PidNotFound(pid))?;
    Ok((image_name(process), cmdline(process), process.start_time()))
}

pub fn get_starttime(pid: u32) -> Option<u64> {
    let sysinfo_pid = Pid::from_u32(pid);
    let system = refresh(ProcessesToUpdate::Some(&[sysinfo_pid]));
    system.process(sysinfo_pid).map(|p| p.start_time())
}

pub fn find_processes_by_name(name: &str, mode: AddMode) -> Result<FindResult, TargetError> {
    let system = refresh(ProcessesToUpdate::All);
    let mut matches = Vec::new();
    let query_lower = name.to_ascii_lowercase();

    for (pid, process) in system.processes() {
        let proc_name = image_name(process);
        let cmd = cmdline(process);

        let is_match = match mode {
            AddMode::Exact => names_equal(&proc_name, name),
            AddMode::Substring => {
                !cmd.is_empty() && cmd.to_ascii_lowercase().contains(&query_lower)
            }
            AddMode::Pid => false,
        };

        if is_match {
            matches.push((pid.as_u32(), proc_name, cmd, process.start_time()));
        }
    }

    Ok(FindResult {
        matches,
        permission_denied_count: 0,
    })
}
