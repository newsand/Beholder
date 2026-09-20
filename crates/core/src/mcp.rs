use crate::buffer::BufferManager;
use crate::model::{AddMode, VramBoardSample, WindowConfig};
use crate::nvml::NvmlWrapper;
use crate::sampler::Sampler;
use crate::targets::TargetManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

const ERR_PID_NOT_FOUND: i32 = -32001;
const ERR_PERMISSION_DENIED: i32 = -32002;
const ERR_GPU_UNAVAILABLE: i32 = -32003;
const ERR_INVALID_INTERVAL: i32 = -32004;
const ERR_INVALID_WINDOW: i32 = -32005;
const ERR_TARGET_NOT_WATCHED: i32 = -32006;
const ERR_NAME_NO_MATCH: i32 = -32007;
const ERR_PARTIAL_PERMISSION: i32 = -32008;
const ERR_INVALID_PARAMS: i32 = -32602;
const ERR_METHOD_NOT_FOUND: i32 = -32601;

struct SharedState {
    targets: TargetManager,
    buffer: BufferManager,
    sampler: Sampler,
    last_vram_board: Option<VramBoardSample>,
}

pub struct McpServer {
    state: Arc<Mutex<SharedState>>,
    nvml: Option<Arc<NvmlWrapper>>,
    ticker_running: Arc<Mutex<bool>>,
}

impl McpServer {
    pub fn new(nvml: Option<NvmlWrapper>) -> Self {
        let nvml = nvml.map(Arc::new);
        Self {
            state: Arc::new(Mutex::new(SharedState {
                targets: TargetManager::new(),
                buffer: BufferManager::default(),
                sampler: Sampler::new(),
                last_vram_board: None,
            })),
            nvml,
            ticker_running: Arc::new(Mutex::new(false)),
        }
    }

    fn start_ticker(&self) {
        let state = Arc::clone(&self.state);
        let nvml = self.nvml.clone();
        let running = Arc::clone(&self.ticker_running);

        {
            let mut r = running.lock().unwrap();
            if *r {
                return;
            }
            *r = true;
        }

        thread::spawn(move || {
            loop {
                let interval_ms = {
                    let s = state.lock().unwrap();
                    s.buffer.config().interval_ms
                };

                thread::sleep(Duration::from_millis(interval_ms as u64));

                {
                    let mut s = state.lock().unwrap();

                    let mut pids_to_sample = Vec::new();
                    for pid in s.targets.alive_pids() {
                        match s.targets.check_pid_status(pid) {
                            crate::targets::PidStatus::Dead => {
                                s.targets.mark_dead(pid);
                            }
                            crate::targets::PidStatus::Reused => {
                                s.targets.mark_reused(pid);
                            }
                            crate::targets::PidStatus::Alive => {
                                pids_to_sample.push(pid);
                            }
                        }
                    }

                    let result = s.sampler.sample_pids(&pids_to_sample, nvml.as_deref());

                    for pid in result.not_found {
                        s.targets.mark_dead(pid);
                    }

                    for sample in result.ram_samples {
                        s.buffer.push_ram(sample, true);
                    }
                    for sample in result.vram_samples {
                        s.buffer.push_vram(sample, true);
                    }

                    if let Some(ref nvml) = nvml {
                        if let Ok(board) = nvml.get_board_vram() {
                            s.last_vram_board = Some(board);
                        }
                    }
                }

                if !*running.lock().unwrap() {
                    break;
                }
            }
        });
    }

    pub fn run(&mut self) {
        self.start_ticker();

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            if line.trim().is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Value::Null,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                    let _ = stdout.flush();
                    continue;
                }
            };

            let response = self.handle_request(&request);
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
            let _ = stdout.flush();
        }

        *self.ticker_running.lock().unwrap() = false;
    }

    fn handle_request(&mut self, req: &JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.clone().unwrap_or(Value::Null);

        if req.method == "initialize" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "beholder",
                        "version": VERSION
                    }
                })),
                error: None,
            };
        }

        if req.method == "notifications/initialized" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({})),
                error: None,
            };
        }

        if req.method == "tools/list" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "tools": self.list_tools()
                })),
                error: None,
            };
        }

        if req.method == "tools/call" {
            let tool_name = req.params.get("name").and_then(|v| v.as_str());
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));

            if let Some(name) = tool_name {
                return self.call_tool(id, name, args);
            } else {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: ERR_INVALID_PARAMS,
                        message: "Missing tool name".to_string(),
                        data: None,
                    }),
                };
            }
        }

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: ERR_METHOD_NOT_FOUND,
                message: format!("Method not found: {}", req.method),
                data: None,
            }),
        }
    }

    fn list_tools(&self) -> Value {
        json!([
            {
                "name": "health",
                "description": "Get system health status",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "list_targets",
                "description": "List all watched targets",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "add_target_pid",
                "description": "Add a target by PID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": { "type": "integer", "description": "Process ID" }
                    },
                    "required": ["pid"]
                }
            },
            {
                "name": "add_target_name",
                "description": "Add targets by name",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Process name or pattern" },
                        "mode": { "type": "string", "enum": ["exact", "substring"], "description": "Match mode" }
                    },
                    "required": ["name", "mode"]
                }
            },
            {
                "name": "remove_target",
                "description": "Remove a target",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": { "type": "integer", "description": "Process ID" }
                    },
                    "required": ["pid"]
                }
            },
            {
                "name": "set_interval_ms",
                "description": "Set sampling interval",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "ms": { "type": "integer", "enum": [100, 250, 500, 1000, 2000] }
                    },
                    "required": ["ms"]
                }
            },
            {
                "name": "set_window_secs",
                "description": "Set window duration",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "secs": { "type": "integer", "enum": [30, 300, 1800, 3600] }
                    },
                    "required": ["secs"]
                }
            },
            {
                "name": "clear_buffer",
                "description": "Clear sample buffer (targets remain)",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_live",
                "description": "Get current metrics for targets (actual latest sample, not downsampled)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": { "type": "integer", "description": "Optional: specific PID" }
                    }
                }
            },
            {
                "name": "get_vram",
                "description": "Get GPU0 board VRAM (total/used/free now)",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "window_stats",
                "description": "Get peak/current/count stats for window (uses downsampled envelope)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": { "type": "integer", "description": "Optional: specific PID" }
                    }
                }
            },
            {
                "name": "export_buffer",
                "description": "Export buffer data",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "format": { "type": "string", "enum": ["csv", "json"] },
                        "pid": { "type": "integer", "description": "Optional: specific PID" },
                        "max_points": { "type": "integer", "description": "Max points (default 1200)" }
                    },
                    "required": ["format"]
                }
            }
        ])
    }

    fn call_tool(&mut self, id: Value, name: &str, args: Value) -> JsonRpcResponse {
        let result = match name {
            "health" => self.tool_health(),
            "list_targets" => self.tool_list_targets(),
            "add_target_pid" => self.tool_add_target_pid(&args),
            "add_target_name" => self.tool_add_target_name(&args),
            "remove_target" => self.tool_remove_target(&args),
            "set_interval_ms" => self.tool_set_interval(&args),
            "set_window_secs" => self.tool_set_window(&args),
            "clear_buffer" => self.tool_clear_buffer(),
            "get_live" => self.tool_get_live(&args),
            "get_vram" => self.tool_get_vram(),
            "window_stats" => self.tool_window_stats(&args),
            "export_buffer" => self.tool_export_buffer(&args),
            _ => Err((ERR_METHOD_NOT_FOUND, format!("Unknown tool: {}", name))),
        };

        match result {
            Ok(content) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&content).unwrap()
                    }]
                })),
                error: None,
            },
            Err((code, message)) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message,
                    data: None,
                }),
            },
        }
    }

    fn tool_health(&self) -> Result<Value, (i32, String)> {
        let state = self.state.lock().unwrap();
        let config = state.buffer.config();
        Ok(json!({
            "version": VERSION,
            "sampler_ok": true,
            "nvml_available": self.nvml.is_some(),
            "window_secs": config.window_secs,
            "interval_ms": config.interval_ms,
            "effective_bucket_ms": config.bucket_ms(),
            "num_cpus": state.sampler.num_cpus(),
            "num_cpus_detected": state.sampler.num_cpus_detected()
        }))
    }

    fn tool_list_targets(&self) -> Result<Value, (i32, String)> {
        let state = self.state.lock().unwrap();
        let targets: Vec<Value> = state
            .targets
            .list()
            .map(|t| {
                json!({
                    "pid": t.pid,
                    "name": t.name,
                    "alive": t.alive,
                    "add_mode": t.add_mode
                })
            })
            .collect();
        Ok(json!({ "targets": targets }))
    }

    fn tool_add_target_pid(&mut self, args: &Value) -> Result<Value, (i32, String)> {
        let pid = args
            .get("pid")
            .and_then(|v| v.as_u64())
            .ok_or((ERR_INVALID_PARAMS, "Missing pid".to_string()))? as u32;

        let mut state = self.state.lock().unwrap();
        match state.targets.add_by_pid(pid) {
            Ok(target) => Ok(json!({
                "ok": true,
                "pid": target.pid,
                "name": target.name
            })),
            Err(e) => {
                let (code, msg) = match &e {
                    crate::targets::TargetError::PidNotFound(_) => (ERR_PID_NOT_FOUND, e.to_string()),
                    crate::targets::TargetError::PermissionDenied(_) => {
                        (ERR_PERMISSION_DENIED, e.to_string())
                    }
                    _ => (ERR_INVALID_PARAMS, e.to_string()),
                };
                Err((code, msg))
            }
        }
    }

    fn tool_add_target_name(&mut self, args: &Value) -> Result<Value, (i32, String)> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((ERR_INVALID_PARAMS, "Missing name".to_string()))?;

        let mode_str = args
            .get("mode")
            .and_then(|v| v.as_str())
            .ok_or((ERR_INVALID_PARAMS, "Missing mode".to_string()))?;

        let mode = match mode_str {
            "exact" => AddMode::Exact,
            "substring" => AddMode::Substring,
            _ => return Err((ERR_INVALID_PARAMS, "Invalid mode".to_string())),
        };

        let mut state = self.state.lock().unwrap();
        match state.targets.add_by_name(name, mode) {
            Ok(pids) => Ok(json!({
                "ok": true,
                "added_pids": pids
            })),
            Err(e) => {
                let (code, msg) = match &e {
                    crate::targets::TargetError::NameNoMatch(_) => (ERR_NAME_NO_MATCH, e.to_string()),
                    crate::targets::TargetError::PartialPermission { .. } => {
                        (ERR_PARTIAL_PERMISSION, e.to_string())
                    }
                    crate::targets::TargetError::PermissionDenied(_) => {
                        (ERR_PERMISSION_DENIED, e.to_string())
                    }
                    _ => (ERR_INVALID_PARAMS, e.to_string()),
                };
                Err((code, msg))
            }
        }
    }

    fn tool_remove_target(&mut self, args: &Value) -> Result<Value, (i32, String)> {
        let pid = args
            .get("pid")
            .and_then(|v| v.as_u64())
            .ok_or((ERR_INVALID_PARAMS, "Missing pid".to_string()))? as u32;

        let mut state = self.state.lock().unwrap();
        state.buffer.remove_target(pid);
        match state.targets.remove(pid) {
            Ok(()) => Ok(json!({ "ok": true })),
            Err(e) => Err((ERR_TARGET_NOT_WATCHED, e.to_string())),
        }
    }

    fn tool_set_interval(&mut self, args: &Value) -> Result<Value, (i32, String)> {
        let ms = args
            .get("ms")
            .and_then(|v| v.as_u64())
            .ok_or((ERR_INVALID_PARAMS, "Missing ms".to_string()))? as u32;

        if !WindowConfig::VALID_INTERVALS.contains(&ms) {
            return Err((ERR_INVALID_INTERVAL, format!("Invalid interval: {}", ms)));
        }

        let mut state = self.state.lock().unwrap();
        let config = state.buffer.config();
        let new_config = WindowConfig::new(config.window_secs, ms).unwrap();
        state.buffer.set_config(new_config);

        Ok(json!({
            "ok": true,
            "interval_ms": ms,
            "effective_bucket_ms": new_config.bucket_ms()
        }))
    }

    fn tool_set_window(&mut self, args: &Value) -> Result<Value, (i32, String)> {
        let secs = args
            .get("secs")
            .and_then(|v| v.as_u64())
            .ok_or((ERR_INVALID_PARAMS, "Missing secs".to_string()))? as u32;

        if !WindowConfig::VALID_WINDOWS.contains(&secs) {
            return Err((ERR_INVALID_WINDOW, format!("Invalid window: {}", secs)));
        }

        let mut state = self.state.lock().unwrap();
        let config = state.buffer.config();
        let new_config = WindowConfig::new(secs, config.interval_ms).unwrap();
        state.buffer.set_config(new_config);

        Ok(json!({
            "ok": true,
            "window_secs": secs,
            "effective_bucket_ms": new_config.bucket_ms()
        }))
    }

    fn tool_clear_buffer(&mut self) -> Result<Value, (i32, String)> {
        let mut state = self.state.lock().unwrap();
        state.buffer.clear();
        Ok(json!({ "ok": true }))
    }

    fn tool_get_live(&self, args: &Value) -> Result<Value, (i32, String)> {
        let filter_pid = args.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);

        let state = self.state.lock().unwrap();
        let mut results = Vec::new();

        for target in state.targets.list() {
            if let Some(pid) = filter_pid {
                if target.pid != pid {
                    continue;
                }
            }

            let ram = state.buffer.get_ram_series(target.pid).and_then(|s| s.latest_raw());
            let vram = state.buffer.get_vram_series(target.pid).and_then(|s| s.latest_raw());

            results.push(json!({
                "pid": target.pid,
                "name": target.name,
                "alive": target.alive,
                "ts_ms": ram.map(|s| s.ts_ms),
                "rss_bytes": ram.map(|s| s.rss_bytes),
                "cpu_pct": ram.map(|s| s.cpu_pct),
                "vram_ts_ms": vram.map(|s| s.ts_ms),
                "vram_bytes": vram.map(|s| s.used_bytes)
            }));
        }

        if let Some(pid) = filter_pid {
            if results.is_empty() {
                return Err((ERR_TARGET_NOT_WATCHED, format!("PID {} not watched", pid)));
            }
        }

        Ok(json!({ "targets": results }))
    }

    fn tool_get_vram(&self) -> Result<Value, (i32, String)> {
        let state = self.state.lock().unwrap();
        match &state.last_vram_board {
            Some(board) => Ok(json!({
                "gpu_index": board.gpu_index,
                "total_bytes": board.total_bytes,
                "used_bytes": board.used_bytes,
                "free_bytes": board.free_bytes
            })),
            None => Err((ERR_GPU_UNAVAILABLE, "GPU not available".to_string())),
        }
    }

    fn tool_window_stats(&self, args: &Value) -> Result<Value, (i32, String)> {
        let filter_pid = args.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);

        let state = self.state.lock().unwrap();
        let mut results = Vec::new();

        for target in state.targets.list() {
            if let Some(pid) = filter_pid {
                if target.pid != pid {
                    continue;
                }
            }

            let ram_series = state.buffer.get_ram_series(target.pid);
            let vram_series = state.buffer.get_vram_series(target.pid);

            let ram_peak = ram_series.and_then(|s| s.peak_rss());
            let ram_latest = ram_series.and_then(|s| s.latest_downsampled());
            let ram_count = ram_series.map(|s| s.count()).unwrap_or(0);

            let vram_peak = vram_series.and_then(|s| s.peak());
            let vram_latest = vram_series.and_then(|s| s.latest_downsampled());
            let vram_count = vram_series.map(|s| s.count()).unwrap_or(0);

            results.push(json!({
                "pid": target.pid,
                "name": target.name,
                "ram": {
                    "peak_bytes": ram_peak.map(|(_, r)| r),
                    "peak_ts_ms": ram_peak.map(|(t, _)| t),
                    "current_rss_bytes": ram_latest.map(|(_, r, _, _)| r),
                    "current_rss_ts_ms": ram_latest.map(|(t, _, _, _)| t),
                    "current_cpu_pct": ram_latest.map(|(_, _, _, c)| c),
                    "current_cpu_ts_ms": ram_latest.map(|(_, _, t, _)| t),
                    "sample_count": ram_count
                },
                "vram": {
                    "peak_bytes": vram_peak.map(|(_, v)| v),
                    "peak_ts_ms": vram_peak.map(|(t, _)| t),
                    "current_bytes": vram_latest.map(|(_, v)| v),
                    "current_ts_ms": vram_latest.map(|(t, _)| t),
                    "sample_count": vram_count
                }
            }));
        }

        if let Some(pid) = filter_pid {
            if results.is_empty() {
                return Err((ERR_TARGET_NOT_WATCHED, format!("PID {} not watched", pid)));
            }
        }

        Ok(json!({
            "window_secs": state.buffer.config().window_secs,
            "targets": results
        }))
    }

    fn tool_export_buffer(&self, args: &Value) -> Result<Value, (i32, String)> {
        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .ok_or((ERR_INVALID_PARAMS, "Missing format".to_string()))?;

        let filter_pid = args.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);
        let max_points = args
            .get("max_points")
            .and_then(|v| v.as_u64())
            .map(|p| p as usize)
            .unwrap_or(WindowConfig::MAX_POINTS);

        let max_points = max_points.min(WindowConfig::MAX_POINTS);

        match format {
            "csv" => self.export_csv(filter_pid, max_points),
            "json" => self.export_json(filter_pid, max_points),
            _ => Err((ERR_INVALID_PARAMS, "Invalid format".to_string())),
        }
    }

    fn export_csv(&self, filter_pid: Option<u32>, max_points: usize) -> Result<Value, (i32, String)> {
        let state = self.state.lock().unwrap();
        let mut lines = vec!["rss_ts_ms,cpu_ts_ms,pid,name,rss_bytes,cpu_pct,vram_ts_ms,vram_bytes".to_string()];

        for target in state.targets.list() {
            if let Some(pid) = filter_pid {
                if target.pid != pid {
                    continue;
                }
            }

            let ram_points = state
                .buffer
                .get_ram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let vram_points = state
                .buffer
                .get_vram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let mut combined: Vec<(u64, u64, u64, f32, Option<(u64, u64)>)> = Vec::new();

            for (rss_ts, rss, cpu_ts, cpu) in &ram_points {
                combined.push((*rss_ts, *cpu_ts, *rss, *cpu, None));
            }

            for (ts, vram) in &vram_points {
                if let Some(entry) = combined.iter_mut().find(|(rss_ts, _, _, _, _)| *rss_ts == *ts) {
                    entry.4 = Some((*ts, *vram));
                } else {
                    combined.push((*ts, *ts, 0, 0.0, Some((*ts, *vram))));
                }
            }

            combined.sort_by_key(|(rss_ts, _, _, _, _)| *rss_ts);

            let skip = combined.len().saturating_sub(max_points);
            for (rss_ts, cpu_ts, rss, cpu, vram) in combined.into_iter().skip(skip) {
                let (vram_ts_str, vram_str) = match vram {
                    Some((ts, v)) => (ts.to_string(), v.to_string()),
                    None => (String::new(), String::new()),
                };
                lines.push(format!(
                    "{},{},{},{},{},{:.2},{},{}",
                    rss_ts, cpu_ts, target.pid, target.name, rss, cpu, vram_ts_str, vram_str
                ));
            }
        }

        Ok(json!({ "csv": lines.join("\n") }))
    }

    fn export_json(&self, filter_pid: Option<u32>, max_points: usize) -> Result<Value, (i32, String)> {
        let state = self.state.lock().unwrap();
        let mut data = Vec::new();

        for target in state.targets.list() {
            if let Some(pid) = filter_pid {
                if target.pid != pid {
                    continue;
                }
            }

            let ram_points = state
                .buffer
                .get_ram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let vram_points = state
                .buffer
                .get_vram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let skip = ram_points.len().saturating_sub(max_points);
            let ram_samples: Vec<Value> = ram_points
                .into_iter()
                .skip(skip)
                .map(|(rss_ts, rss, cpu_ts, cpu)| {
                    json!({
                        "rss_ts_ms": rss_ts,
                        "rss_bytes": rss,
                        "cpu_ts_ms": cpu_ts,
                        "cpu_pct": cpu
                    })
                })
                .collect();

            let skip = vram_points.len().saturating_sub(max_points);
            let vram_samples: Vec<Value> = vram_points
                .into_iter()
                .skip(skip)
                .map(|(ts, vram)| {
                    json!({
                        "ts_ms": ts,
                        "used_bytes": vram
                    })
                })
                .collect();

            data.push(json!({
                "pid": target.pid,
                "name": target.name,
                "ram_samples": ram_samples,
                "vram_samples": vram_samples
            }));
        }

        Ok(json!({ "data": data }))
    }
}
