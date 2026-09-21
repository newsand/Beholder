use super::FindResult;
use crate::model::AddMode;
use crate::targets::TargetError;
use std::fs;
use std::path::Path;

pub fn read_process_info(pid: u32) -> Result<(String, String, u64), TargetError> {
    let proc_path = Path::new("/proc").join(pid.to_string());

    if !proc_path.exists() {
        return Err(TargetError::PidNotFound(pid));
    }

    let comm_path = proc_path.join("comm");
    let name = fs::read_to_string(&comm_path)
        .map_err(|_| TargetError::PermissionDenied(pid))?
        .trim()
        .to_string();

    let cmdline_path = proc_path.join("cmdline");
    let cmdline = fs::read_to_string(&cmdline_path)
        .map_err(|_| TargetError::PermissionDenied(pid))?
        .replace('\0', " ")
        .trim()
        .to_string();

    let stat_path = proc_path.join("stat");
    let stat = fs::read_to_string(&stat_path).map_err(|_| TargetError::PermissionDenied(pid))?;
    let starttime = parse_starttime(&stat).ok_or(TargetError::PermissionDenied(pid))?;

    Ok((name, cmdline, starttime))
}

fn parse_starttime(stat: &str) -> Option<u64> {
    let end_paren = stat.rfind(')')?;
    let after_paren = &stat[end_paren + 2..];
    let fields: Vec<&str> = after_paren.split_whitespace().collect();
    fields.get(19)?.parse().ok()
}

pub fn get_starttime(pid: u32) -> Option<u64> {
    let stat_path = Path::new("/proc").join(pid.to_string()).join("stat");
    let stat = fs::read_to_string(&stat_path).ok()?;
    parse_starttime(&stat)
}

pub fn find_processes_by_name(name: &str, mode: AddMode) -> Result<FindResult, TargetError> {
    let mut matches = Vec::new();
    let mut permission_denied_count = 0;

    let proc_dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => {
            return Ok(FindResult {
                matches,
                permission_denied_count,
            })
        }
    };

    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();

        if let Ok(pid) = pid_str.parse::<u32>() {
            match read_process_info(pid) {
                Ok((proc_name, cmdline, starttime)) => {
                    let is_match = match mode {
                        AddMode::Exact => proc_name == name,
                        AddMode::Substring => cmdline.contains(name),
                        AddMode::Pid => false,
                    };

                    if is_match {
                        matches.push((pid, proc_name, cmdline, starttime));
                    }
                }
                Err(TargetError::PermissionDenied(_)) => {
                    permission_denied_count += 1;
                }
                Err(_) => {}
            }
        }
    }

    Ok(FindResult {
        matches,
        permission_denied_count,
    })
}
