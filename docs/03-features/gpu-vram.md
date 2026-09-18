# Feature: VRAM NVIDIA (total + por processo)

## Depende de
- feature-add-target
- feature-sampler (**mesmo processo OS**)

## Descrição
**Job do MVP** (junto com RAM). Fonte: **NVML in-process** (não CLI).
- **Placa** (total/used/free): estado **agora** (sem janela).
- **Por processo/alvo**: série na **mesma janela** 30s–1h dos gráficos de histórico (caça leak de VRAM).

## Fluxo
- Dado NVML ok → amostras de placa + por alvo no ticker
- Dado `--mcp` → tools de VRAM com os mesmos números da GUI

## Casos de erro
- NVML/driver ausente → "GPU indisponível"; RAM segue
- PID sem memória GPU → 0 ou N/A explícito
- Multi-GPU → MVP GPU 0

## Não fazer
- HTTP; segundo processo; parse contínuo de `nvidia-smi`
- Aceite vago só por "ordem de grandeza"

## Critério de aceite
- [ ] total/used/free via NVML no intervalo de amostragem
- [ ] VRAM por PID alinhada ao NVML; retida na janela (pico/série)
- [ ] Sem GPU: sem crash; RAM ok
- [ ] Tools MCP = mesmos campos da GUI
