# Non-negotiables (Beholder)

- Stack: Rust + egui + egui_plot + sysinfo + wgpu. Sem WebView/JS/Tauri/Electron.
- Módulos/features no **mesmo binário** — proibido hexagonal/clean/DDD/repository; proibido 2 processos OS.
- Agentes = **MCP `--mcp`** only. Zero HTTP. Paridade GUI ↔ tools MCP.
- MCP headless (sem janela) é modo MVP do mesmo binário.
- Métricas com nome nativo (RSS, Working Set, Private Bytes, VRAM…).
- Só alvos pedidos — sem inventário global como home.
- VRAM é **job do MVP**; fonte **NVML in-process**; não CLI periódica.
- Sem GPU NVIDIA: app continua (RAM/CPU); VRAM indisponível — não crash.
- Pico = max da **janela** atual. Timestamp monotônico. PID reuse via starttime.
- Export CSV/JSON plotável sem limpeza manual.
- Features com erro explícito + aceite testável; BUILD só após Moriaty + Socrates.
- SYNC: código e `/docs` no mesmo commit quando o comportamento muda.

- Vocabulário de produto: **janela/buffer** — nunca "sessão".
- Um Model central (Elm/UDF); sampler emite events/samples.
- Ring buffer: máx. **1200** pts/série; downsample = **max por bucket**; UI/DOC admitem gráfico = envelope de picos; ts da amostra real; monitor não é o vilão.
- `get_vram` = só placa agora; histórico VRAM-por-pid na janela via live/stats/export.
