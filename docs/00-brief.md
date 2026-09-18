# Projeto: Beholder

## Objetivo em 1 parágrafo
Aplicativo **desktop** local (**Linux no MVP**; Windows depois) para medição precisa do que o usuário apontou: processos (RAM/CPU) e, no mesmo produto, **VRAM NVIDIA** (placa + por processo) para deploy/teste de IA. Desenvolvedores, SREs e agentes de IA (via **MCP**) veem as mesmas métricas — sem inventário de todos os processos e sem distorcer a medição.

## Job do MVP (uma linha — fechado)
**MVP = RAM/CPU dos alvos + VRAM NVIDIA GPU0 (placa=agora; por processo=na janela), GUI Dashboard + MCP `--mcp`, Linux only.** Windows/Widget = fase 2.

## Stack obrigatória
- Linguagem: **Rust**
- UI: **egui** + **egui_plot** + **wgpu** (Vulkan / DX12 / OpenGL fallback)
- Coleta processo: **sysinfo** + `/proc` / Win32 quando necessário
- Coleta VRAM: **NVML in-process** (não parsear CLI `nvidia-smi` a cada sample)
- Agentes: **MCP** com flag **`--mcp`** (stdio) — **zero HTTP/REST/POST**
- Sem WebView, sem JS, sem Tauri/Electron


## Arquitetura de UI/estado (travada — 2026-09-17)
- **State-driven + Unidirectional Data Flow (Elm-style):** Model + Message/Action + Update + View.
- UI = função do estado atual; ações → update → novo estado → view.
- Eventos do sampler alimentam o Model (não MVC/MVVM).
- Actor/event-driven **internos** ok só como detalhe do sampler; não viram camada de "controllers".
- **Proibido:** MVC, MVVM, hexagonal/clean/DDD.

## Arquitetura de execução (travada)
- **Um único binário / um único processo OS:** sampler + egui **ou** sampler + MCP no mesmo programa. Crates internas ok; **dois executáveis = proibido**.
- `--mcp` = modo **headless MCP** (sem janela) — é MVP, não "fora".
- Paridade: o que a GUI mostra deve ser consultável via **tools MCP** (não via API HTTP).

## Público
Desenvolvedores de microserviços / IA, agentes de IA, caçadores de leak (RAM e VRAM).

## Metas do MVP (v1)
- Adicionar alvo por PID, nome **exato** ou **substring em cmdline** (sem re-scan)
- Gráfico RAM residente: Linux **RSS** (Windows métricas = fase 2)
- **VRAM NVIDIA:** placa = total/used/free **agora**; por processo/alvo = série **na mesma janela** dos gráficos de histórico (caça leak de VRAM)
- Intervalos: 100 ms, 250 ms, 500 ms, 1 s, 2 s (VRAM no mesmo ticker; se NVML custar, discretion: VRAM ≥ 250 ms)
- Tabela: PID, nome, RAM atual, pico **na janela**, CPU %; VRAM placa (agora) + VRAM por processo **na janela**
- Processo morto marcado; amostras na janela permanecem até limpar/expirar
- Export CSV/JSON (RAM + VRAM quando houver)
- Janela **Dashboard** (MVP); Widget = fase 2; always-on-top opcional no Dashboard
- Modo **`--mcp`**: tools equivalentes sem HTTP
- Nomes de métrica nativos na UI
- Sem listar todos os processos como tela inicial

## Não-metas (explícito)
- NÃO macOS no MVP
- NÃO Windows no MVP (fase 2)
- NÃO Widget/overlay como UI default (fase 2; MVP = Dashboard)
- NÃO monitoramento remoto / Prometheus / APM
- NÃO HTTP/REST/POST / bind de porta
- NÃO dois processos OS (GUI vs sampler)
- NÃO parse periódico de CLI `nvidia-smi` como fonte de verdade
- NÃO profiling CUDA completo / flamegraph / kernels
- NÃO disco / rede no MVP
- NÃO kill / priority / affinity
- NÃO telemetria saindo da máquina
- NÃO Docker monitoring no MVP
- NÃO filhos automáticos / alertas avançados / multi-GPU fancy — v2+ (default GPU 0)

## Decisões fechadas (sala — 2026-09-17)
- Desktop local; agentes = **MCP `--mcp`** only (zero HTTP).
- Um processo; MCP headless = MVP.
- VRAM NVIDIA = **job do MVP**; fonte **NVML in-process**; **GPU 0** only.
- Paridade GUI ↔ tools MCP (I9).
- Vocabulário: **janela/buffer** — nunca "sessão".
- UI app: **um Model** + UDF/Elm; sampler só emite samples/events.
- Piso MCP: **B** em `04-contracts.md`: `get_vram`=placa agora; `get_live`/`window_stats`=RAM+VRAM-por-alvo; `export_buffer` com teto/recorte.
- Licença: **MIT**.
- Plataforma MVP: **Linux only** (Windows fase 2).
- UI MVP: **Dashboard** (Widget fase 2).

## Amostragem / janela (real-time)
- Sistema **real-time**. Vocabulário: **janela/buffer de amostras** — **proibido** chamar de "sessão" (não é login nem gravação start/stop de negócio).
- **Sem re-scan:** alvos entram só no add; novos PIDs com o mesmo nome **não** entram sozinhos depois.
- Busca: **PID** | **nome exato** (`comm`/nome curto) | **substring no cmdline completo** (não só comm de 15 chars). Case-sensitive no Linux.
- Gráficos **com histórico:** janela selecionável **30 s | 5 min | 30 min | 1 h** (ring buffer time-based). Pico = max **nessa janela**.
- Gráficos/painéis **só estado atual** (ex.: VRAM total used/free "agora"): **sem** janela — só última amostra.
- Limpar = limpa o **buffer/janela** do gráfico; **alvos permanecem**.

## Aberto (não bloqueiam BUILD se defaults abaixo)
- Nome comercial final (binário `beholder` provisório ok)
- Confirmar: match case-sensitive no Linux (default = sim, nativo)

## Decisões de produto (fechadas agora)
- Sem re-scan
- Add: PID | nome exato | substring
- Janela de gráfico: 30s / 5min / 30min / 1h onde histórico importa
- Demais: só estado atual real-time
- Limpar = buffer/janela; alvos ficam
- MCP piso **B** com nomes `window_*` / `buffer_*` (não `session_*`)

## Diferido (fase 2 — não bloqueia Linux MVP)
- Windows (Working Set / Private Bytes)
- Widget UI
- Multi-GPU além de GPU 0
- Nome comercial final (binário pode continuar beholder provisório)

## Fonte
Produto v0.2 + decisões da sala.
