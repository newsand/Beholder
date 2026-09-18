# Arquitetura

## Execução
- Um binário, um processo OS.
- Modos: GUI Dashboard (default) | `--mcp` headless stdio.
- Crates sugeridas: `core` (sampler, model, nvml), `app` (eframe/egui) — mesmo processo.

## UI / estado (travado)
```
Message/Action ──► Update(Model) ──► Model ──► View(egui)
                         ▲
Sampler/NVML events ─────┘
```
- Um Model central (Elm / UDF / state-driven).
- Sampler e NVML só emitem samples/events; não possuem "UI state" paralelo.
- Proibido: MVC, MVVM, hexagonal, 2º Model por alvo (Actor como padrão de app).

## Pastas (MVP Linux)
```
/src or /crates
  core/   sampler, targets, buffer/window, nvml, mcp tools
  app/    egui dashboard, plots
```
Nomes exatos = discretion; boundaries acima = não-negociável.

## Dados
- Ring buffer por série com retenção = `window_secs`.
- Séries "atual only" guardam último ponto.
- Export = dump do buffer.
