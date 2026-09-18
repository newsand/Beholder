# Discretion — o que o agente PODE decidir sozinha

- Nomes de crates/módulos internos (mesmo binário).
- Layout egui respeitando superfície mínima.
- Ring buffer / flush JSONL (sem mentir timestamp).
- Mensagens de erro claras.
- Cores de série; tema técnico limpo.
- Fallback OpenGL se wgpu falhar — com mensagem.
- Se NVML custo alto: amostrar VRAM em intervalo ≥ 250 ms mesmo com RAM a 100 ms.
- Multi-GPU: **default GPU 0** se brief não fechar outra regra.

# O que NÃO pode decidir sozinha

- Reintroduzir HTTP/REST/POST.
- Separar sampler e UI em dois executáveis/processos OS.
- Tirar VRAM do MVP ou tratar como spike sem mudar o brief.
- Usar parse de `nvidia-smi` CLI como sampler contínuo.
- Adicionar macOS, remoto, disco/rede, kill/priority, Docker, full CUDA profiler.
- Quebrar paridade GUI ↔ tools MCP.
- Somar RSS pai+filhos sem série explícita.
