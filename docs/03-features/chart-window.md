# Feature: Gráfico, janela e buffer (real-time)

## Depende de
- feature-sampler

## Descrição
Dashboard egui_plot. Séries **com histórico** usam janela selecionável **30s / 5min / 30min / 1h**. Painéis **só agora** (ex. VRAM placa) mostram última amostra, sem janela. Série plotada = **max por bucket** (≤1200 pts): gráfico = **envelope de máximos**, não curva sample-a-sample. Ts da amostra do max. `window_stats.peak` = max da janela (coincide com envelope). Limpar buffer limpa amostras; alvos ficam. Export CSV/JSON do buffer.

Vocabulário: **nunca "sessão"** (usar janela/buffer).

## Casos de erro
- Export buffer vazio → CSV só header ou erro explícito (default: header)
- Dois alvos → cores distintas

## Critério de aceite
- [ ] Trocar janela 30s→5min altera o range do eixo X / retenção
- [ ] Limpar buffer zera o plot; alvos continuam amostrando
- [ ] Export replotável sem limpeza manual
- [ ] Painel "atual" não depende da janela
