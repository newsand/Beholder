# Beholder

![Beholder](assets/wallpaper.jpg)

Desktop process monitor with NVIDIA VRAM tracking (Linux and Windows). Designed for developers, SREs, and AI agents hunting memory leaks in RAM and VRAM.

## Features

- **Process Monitoring**: Track RAM (Linux RSS / Windows Working Set) and CPU% for targeted processes
- **NVIDIA VRAM**: GPU 0 board memory (total/used/free) and per-process VRAM usage
- **Flexible Targeting**: Add processes by PID, exact name (`comm`), or cmdline substring
- **Time Windows**: 30s, 5min, 30min, or 1h history with automatic downsampling
- **MCP Integration**: Headless mode for AI agent integration via stdio
- **Export**: CSV and JSON export of collected metrics

## Build

Requires Rust 1.75+. Linux or Windows (MSVC).

```bash
cargo build --release
```

The binary will be at `target/release/beholder` (Linux) or `target/release/beholder.exe` (Windows).

### Windows production release

Run on Windows (PowerShell), with Visual Studio C++ Build Tools installed:

```powershell
.\scripts\build-windows-release.ps1
```

Writes `dist/windows/beholder.exe` and a versioned zip.

### Dependencies

- **NVIDIA GPU** (optional): Requires NVIDIA driver with NVML library. Without NVIDIA GPU, RAM/CPU monitoring works; VRAM features show "GPU: N/A".

## Run

### GUI Dashboard (default)

```bash
./beholder
```

### MCP Headless Mode

```bash
./beholder --mcp
```

In MCP mode, Beholder reads JSON-RPC requests from stdin and writes responses to stdout. See `docs/04-contracts.md` for the full MCP tool specification.

## Usage

### Adding Targets

1. **By PID**: Enter a process ID directly
2. **By Exact Name**: Match the process `comm` (Linux) or image name (Windows, `.exe` optional)
3. **By Substring**: Match any part of the full cmdline

Targets are added once at the time of the request. New processes with the same name that start later are not automatically tracked (no re-scan).

### Time Windows

Select window duration in the top panel:
- **30s**: High resolution, recent data
- **5min**: Short-term trends
- **30min**: Medium-term analysis
- **1h**: Long-term leak hunting

### Downsampling

To cap memory usage at 1200 points per series:
- **RAM and VRAM**: `max` per bucket (graph shows peak envelope)
- **CPU%**: `last` per bucket

Timestamps are from the actual sample that produced the max/last value, never interpolated.

### Sampling Intervals

Available intervals: 100ms, 250ms, 500ms, 1s, 2s

Effective bucket size: `max(interval_ms, window_ms / 1200)`

### Export

Export collected data via the GUI menu or MCP `export_buffer` tool. Formats:
- **CSV**: `ts_ms,pid,name,rss_bytes,cpu_pct,vram_bytes`
- **JSON**: Structured with separate `ram_samples` and `vram_samples` arrays per target

## MCP Tools

| Tool | Description |
|------|-------------|
| `health` | Version, sampler status, NVML availability, current config |
| `list_targets` | All watched targets with alive status |
| `add_target_pid` | Add target by PID |
| `add_target_name` | Add targets by name (exact or substring mode) |
| `remove_target` | Stop watching a target |
| `set_interval_ms` | Change sampling interval |
| `set_window_secs` | Change history window duration |
| `clear_buffer` | Clear sample history (targets remain) |
| `get_live` | Current metrics for targets (RAM + VRAM) |
| `get_vram` | GPU 0 board VRAM (total/used/free now) |
| `window_stats` | Peak/current/count stats for the window |
| `export_buffer` | Export buffer as CSV or JSON |

## Metrics Glossary

| Metric | Description |
|--------|-------------|
| **RSS** | Resident Set Size - physical memory currently in RAM (Linux `VmRSS`) |
| **Working Set** | Physical memory currently in RAM (Windows; same `rss_bytes` field) |
| **CPU%** | CPU usage percentage, normalized by number of cores |
| **VRAM Total** | Total GPU memory available |
| **VRAM Used** | GPU memory currently in use (board-wide) |
| **VRAM Free** | GPU memory available |
| **VRAM (process)** | GPU memory used by a specific process |
| **Peak** | Maximum value within the current time window |

## Architecture

- Single binary, single process
- Rust + egui + egui_plot + sysinfo + NVML
- Elm-style unidirectional data flow (Model → Update → View)
- Ring buffer with time-based eviction and point cap

See `docs/` for detailed specifications.

## License

MIT
