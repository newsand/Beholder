## [2026-09-17] MODO DOC iniciado

### Construído (doc)
- `00-brief.md` (rascunho)
- `05-non-negotiables.md`, `06-discretion.md`
- Features: add-target, sampler, chart-session
- `INCONSISTENCIAS.md`

### Decisões tomadas fora da doc de produto
- Nenhum comportamento de runtime inventado; gaps listados como perguntas.

### Pendências
- Respostas às perguntas 1–5 do brief
- Completar `01-architecture`, `02-data-models`, `04-contracts`, features restantes MVP

## [2026-09-17] Sala: MCP only + single process + VRAM

### Decisões
- Comunicação agentes = MCP (`--mcp`); HTTP/POST descartados.
- Sampler e egui no **mesmo** processo/binário (crates internas ok).
- Adicionar monitoramento VRAM NVIDIA (total + por processo), estilo nvidia-smi.

### Pendências
- Multi-GPU selector; fonte exata NVML vs parse nvidia-smi (discretion sugeriu NVML se disponível).

## [2026-09-17] Limpeza ataque Moriaty/Socrates

### Construído
- Brief: job MVP = RAM/CPU alvos + VRAM; MCP headless; NVML; 1 processo; I9 = MCP

### Pendências DOC
- Nome/licença, Private Bytes, re-scan nome, UI default, multi-GPU

## [2026-09-17] Respostas Filipe + opções MCP

### Decisões
- Licença MIT; Linux-only MVP; Dashboard; GPU 0; Widget/Windows fase 2
- MCP: opções A/B/C documentadas (ainda não escolhidas)

### Pendências
- Re-scan nome, match nome, clear sessão, pacote MCP A/B/C

## [2026-09-17] Arquitetura UI + glossário amostragem

### Decisões
- UI: State-driven / UDF / Elm (Model-Message-Update-View); sem MVC/MVVM
- "Sessão" reframed = buffer temporal time-based (real-time)

### Pendências
- Re-scan, match, limpar buffer (perguntas reescritas em linguagem clara)

## [2026-09-17] Filipe: sem re-scan; match; janela time-based

### Decisões
- Sem re-scan; add PID|exact|substring
- Janela 30s/5m/30m/1h nos gráficos com histórico; demais = estado atual
- Limpar = buffer; alvos ficam
- Vocabulário sessão→janela/buffer; MCP B congelado em 04-contracts.md
- Um Model Elm/UDF

### Pronto para
- Ataque Moriaty + liberação Socrates no pacote DOC (ainda faltam 01-architecture, 02-data-models finos se exigirem checklist harness)

## [2026-09-17] Limpeza zumbis sessão + cmdline + VRAM janela

### Construído
- Varreu "sessão" do brief/05/features; `feature-chart-window.md`
- Substring = cmdline; exact = comm
- VRAM processo na janela; placa = agora
- MCP B sem "pendente Filipe"

## [2026-09-18] Plenário: teto 3600 + export + split VRAM tools

### Decisões propostas (aguardando Filipe)
- Max 3600 pts/série + downsample retenção
- export_buffer com pid?/max_points?
- get_vram = placa agora; get_live/window_stats = RAM + VRAM-por-alvo

## [2026-09-18] Downsample = last + peak separado

### Decisões
- Max-only na série rejeitado (mente slope)
- Série = last por bucket; peak = window_stats/UI separado
- Ts da amostra real; recalc bucket pra frente

## [2026-09-18] Filipe: max por período + teto 1200

### Decisões
- Downsample = **max por bucket** (não last)
- Gráfico documentado como **envelope de máximos**
- Teto **1200** pontos/série

## [2026-09-18] BUILD iniciado

### Construído
- Documentação completa em `/docs/`
- Cargo workspace: `core` + `app`
- Ring buffer com downsample (max para RAM/VRAM, last para CPU%)
- Sampler via sysinfo (RSS, CPU%)
- NVML wrapper (graceful degradation)
- MCP stdio tools (contracts B)
- GUI Dashboard: add targets, RSS/VRAM plots, table, clear, export
- `--mcp` headless mode

### Decisões tomadas fora da doc de produto
- `nvml-wrapper` crate for NVML bindings (in-process, not CLI parse)
- `serde_json` for MCP JSON-RPC over stdio
- `clap` for CLI args parsing
- Color palette: distinct hues per target series (discretion: technical clean theme)
- Export filename: `beholder_export_{timestamp}.{csv|json}` (discretion)
- MCP error codes mapped to JSON-RPC error codes (-32000 range for app errors)
