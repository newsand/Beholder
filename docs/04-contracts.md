# Contratos MCP (piso B — congelado para DOC)

Transporte: stdio MCP, flag `--mcp`. Zero HTTP. Paridade com Dashboard.

Vocabulário: **window/buffer**, nunca `session`.

## Tools

| Tool | Params | Resultado |
|---|---|---|
| `health` | — | versão, sampler ok, NVML ok/indisp., window_secs atual |
| `list_targets` | — | alvos (pid, nome, alive) |
| `add_target_pid` | `pid: u32` | ok / erro |
| `add_target_name` | `name: string`, `mode: exact\|substring` | exact=`comm`; substring=**cmdline**; só vivos agora |
| `remove_target` | `pid: u32` | ok |
| `set_interval_ms` | `ms: 100\|250\|500\|1000\|2000` | ok |
| `set_window_secs` | `secs: 30\|300\|1800\|3600` | janela dos gráficos com histórico |
| `clear_buffer` | — | limpa amostras da janela; **alvos permanecem** |
| `get_live` | `pid?: u32` | **agora**: RAM + VRAM-por-alvo (se NVML); sem pid = todos |
| `get_vram` | — | **só placa GPU0 agora** (total/used/free) — não histórico |
| `window_stats` | `pid?: u32` | pico/atual/count na **janela** (RAM + VRAM-por-processo) |
| `export_buffer` | `format: csv\|json`, `pid?: u32`, `max_points?: u32` | payload recortado; default `max_points` = 1200 (teto) |

## Erros
`pid_not_found`, `permission_denied`, `gpu_unavailable`, `invalid_interval`, `invalid_window`, `target_not_watched`, `name_no_match`

## Fora do MVP MCP
- `watch` lote (opção C), streams, kill, multi-GPU, HTTP

## Retenção / teto (anti bomba de memória)
- Janela em **tempo** (30s|5m|30m|1h) + ticker (100–2000 ms).
- **Teto duro:** máx. **1200 pontos por série** (RAM ou VRAM-por-pid).
- `bucket_ms = max(interval_ms, window_secs * 1000 / 1200)`.
- Por bucket: **RAM e VRAM-por-alvo = max**; **CPU % = last**. Gráfico de memória = envelope de máximos; CPU não é envelope de spike.
- **Documento honesto:** o gráfico é **envelope de máximos por bucket**, não a curva sample-a-sample. Slope fino entre samples dentro do bucket **não** é representável.
- Timestamp = `ts` da amostra que foi o **máximo** do bucket — nunca centro inventado.
- Troca de janela/intervalo: recalcula `bucket_ms` pra frente; pontos fora da janela caem; sem reescrever densidades passadas.
- UI/MCP: `interval_ms` + `effective_bucket_ms` quando diferirem.

## Export
- `export_buffer` nunca despeja 1h×N PIDs sem freio: obrigatório respeitar teto; params `pid?` e `max_points?` (default = min(1200, pontos na janela)).
