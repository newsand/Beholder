use crate::model::{AddMode, Target};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TargetError {
    #[error("pid_not_found: PID {0} does not exist")]
    PidNotFound(u32),
    #[error("permission_denied: cannot read process {0}")]
    PermissionDenied(u32),
    #[error("name_no_match: no process matches '{0}'")]
    NameNoMatch(String),
    #[error("target_not_watched: PID {0} is not being watched")]
    TargetNotWatched(u32),
}

pub struct TargetManager {
    targets: HashMap<u32, Target>,
}

impl TargetManager {
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
        }
    }

    pub fn add_by_pid(&mut self, pid: u32) -> Result<&Target, TargetError> {
        if self.targets.contains_key(&pid) {
            return Ok(self.targets.get(&pid).unwrap());
        }

        let (name, cmdline, starttime) = read_process_info(pid)?;
        let target = Target::new(pid, name, cmdline, starttime, AddMode::Pid);
        self.targets.insert(pid, target);
        Ok(self.targets.get(&pid).unwrap())
    }

    pub fn add_by_name(&mut self, name: &str, mode: AddMode) -> Result<Vec<u32>, TargetError> {
        let matches = find_processes_by_name(name, mode)?;
        if matches.is_empty() {
            return Err(TargetError::NameNoMatch(name.to_string()));
        }

        let mut added = Vec::new();
        for (pid, proc_name, cmdline, starttime) in matches {
            if !self.targets.contains_key(&pid) {
                let target = Target::new(pid, proc_name, cmdline, starttime, mode);
                self.targets.insert(pid, target);
                added.push(pid);
            }
        }
        Ok(added)
    }

    pub fn remove(&mut self, pid: u32) -> Result<(), TargetError> {
        if self.targets.remove(&pid).is_some() {
            Ok(())
        } else {
            Err(TargetError::TargetNotWatched(pid))
        }
    }

    pub fn get(&self, pid: u32) -> Option<&Target> {
        self.targets.get(&pid)
    }

    pub fn get_mut(&mut self, pid: u32) -> Option<&mut Target> {
        self.targets.get_mut(&pid)
    }

    pub fn list(&self) -> impl Iterator<Item = &Target> {
        self.targets.values()
    }

    pub fn alive_pids(&self) -> Vec<u32> {
        self.targets
            .values()
            .filter(|t| t.alive)
            .map(|t| t.pid)
            .collect()
    }

    pub fn mark_dead(&mut self, pid: u32) {
        if let Some(target) = self.targets.get_mut(&pid) {
            target.alive = false;
        }
    }

    pub fn check_alive(&mut self, pid: u32) -> bool {
        if let Some(target) = self.targets.get(&pid) {
            if !target.alive {
                return false;
            }
            if let Ok((_, _, starttime)) = read_process_info(pid) {
                if starttime == target.starttime {
                    return true;
                }
            }
            self.mark_dead(pid);
            false
        } else {
            false
        }
    }
}

impl Default for TargetManager {
    fn default() -> Self {
        Self::new()
    }
}

fn read_process_info(pid: u32) -> Result<(String, String, u64), TargetError> {
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

fn find_processes_by_name(
    name: &str,
    mode: AddMode,
) -> Result<Vec<(u32, String, String, u64)>, TargetError> {
    let mut results = Vec::new();

    let proc_dir = match fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return Ok(results),
    };

    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let pid_str = file_name.to_string_lossy();

        if let Ok(pid) = pid_str.parse::<u32>() {
            if let Ok((proc_name, cmdline, starttime)) = read_process_info(pid) {
                let matches = match mode {
                    AddMode::Exact => proc_name == name,
                    AddMode::Substring => cmdline.contains(name),
                    AddMode::Pid => false,
                };

                if matches {
                    results.push((pid, proc_name, cmdline, starttime));
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_by_pid_self() {
        let mut manager = TargetManager::new();
        let pid = std::process::id();
        let result = manager.add_by_pid(pid);
        assert!(result.is_ok());
        let target = result.unwrap();
        assert_eq!(target.pid, pid);
        assert!(target.alive);
    }

    #[test]
    fn test_add_invalid_pid() {
        let mut manager = TargetManager::new();
        let result = manager.add_by_pid(999999999);
        assert!(matches!(result, Err(TargetError::PidNotFound(_))));
    }

    #[test]
    fn test_remove_target() {
        let mut manager = TargetManager::new();
        let pid = std::process::id();
        manager.add_by_pid(pid).unwrap();
        assert!(manager.remove(pid).is_ok());
        assert!(manager.get(pid).is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut manager = TargetManager::new();
        let result = manager.remove(999999999);
        assert!(matches!(result, Err(TargetError::TargetNotWatched(_))));
    }
}
