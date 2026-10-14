# Feature: Atalhos de plot (F4 / F5)

## 4 answers (user)
1. How do you describe the feature?
> preciso de 2 shortcuts no projeto. f4 ele recentraliza o grafico. hoje quando eu dou e tiro zoom no grafico ele perde a referencia. preciso que a referencia volte. a segunda feature que eu quero é no F5 quero um refresh no grafico. ele zera e começa de novo . e funciona igual o f4 ele volta para a referencia original de grafico. quero que ele volte a acompanhar a janela

2. What problem does it objectively solve?
> depois de zoom/pan o gráfico perde a referência e deixa de acompanhar a janela ao vivo; também precisa recomeçar o histórico limpo sem ficar preso no transform

3. What is the expected solution? What trade-offs does it involve?
> módulo commands no app; F4 só reset de vista; F5 reusa clear do buffer + mesmo reset. Trade-off: vista é UI (PlotMemory), não Model nem MCP

4. What example or context do we have of the problem and the solution?
> zooma, perde referência → F4 devolve auto-follow; F5 esvazia séries e volta a acompanhar a janela

## Depende de
- feature-chart-window (plots egui_plot + ClearBuffer)

## Comportamento
| Tecla | Efeito |
|---|---|
| F4 | Reseta a vista dos plots (`rss_plot`, `vram_plot`) → auto-bounds / acompanhar janela. **Não** apaga amostras. |
| F5 | `ClearBuffer` (amostras zeradas; alvos permanecem) **+** o mesmo reset de vista do F4. |

Ignorar atalho se o contexto quer keyboard input (ex. campo Add focado).

Botão Clear da UI: só limpa buffer; **não** reseta a vista (salvo pedido futuro).

Vista = estado de UI (`egui_plot` PlotMemory). Sem tool MCP nova: F5 = `clear_buffer` já existente; F4 é só viewport.

## Critério de aceite
- [ ] F4 após zoom restaura auto-bounds em RSS e VRAM; dados intactos
- [ ] F5 zera buffer (igual Clear) e restaura auto-bounds; alvos seguem vivos
- [ ] Com input de texto focado, F4/F5 não disparam
