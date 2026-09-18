# Feature: Adicionar alvo

## Depende de
- (nenhuma)

## Descrição
Adiciona por **PID**, **nome exato** (`comm`/nome curto) ou **substring no cmdline completo**. **Sem re-scan.**

## Fluxo
- PID válido → alvo entra e amostragem começa
- Nome + mode exact|substring → cada match vivo vira alvo; zero match → erro `name_no_match`

## Casos de erro
- PID inexistente / sem permissão → erro claro
- Nome sem match → não cria alvos vazios
- PID reuse → starttime (feature-sampler)

## Critério de aceite
- [ ] PID válido aparece na lista em ≤ 1 intervalo
- [ ] substring encontra múltiplos vivos; não adiciona nascidos depois (sem re-scan)
- [ ] exact não pega substring parcial
- [ ] substring casa cmdline completo, não só comm truncado em 15 chars
