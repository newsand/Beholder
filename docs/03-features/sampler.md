# Feature: Sampler

## Depende de
- feature-add-target

## Descrição
Loop de amostragem com intervalo configurável (100 ms, 250 ms, 500 ms, 1 s, 2 s). Coleta memória residente (e CPU %) sem bloquear a UI. Backpressure: descarta frames **visuais**, nunca mente timestamp gravado.

## Fluxo
- Dado alvos vivos e intervalo I
- Quando o ticker dispara
- Então grava amostra (timestamp mono, métrica nativa, CPU%) no buffer/janela

## Métricas MVP (Linux-only)
- Linux: **RSS** (padrão no gráfico)
- PSS: sob demanda / não default a 100 ms (v2+ ou toggle)

## Casos de erro
- `/proc` some / processo morto → marca death timestamp; série encerra; amostras na janela ficam
- PID reuse → detectar via starttime; não continuar série do morto
- Amostra atrasada → mantém timestamp real; não inventar ponto "no horário ideal"

## Critério de aceite
- [ ] Com intervalo 250 ms, gráfico RSS atualiza em ≤ 1 s após add PID
- [ ] Processo morto fica marcado; amostras na janela permanecem
- [ ] App ocioso 0 alvos: RSS próprio estável (sem leak óbvio do monitor)
