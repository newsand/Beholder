# Data models (MVP)

## Target
| campo | tipo | regra |
|---|---|---|
| pid | u32 | obrigatório |
| name | string | snapshot no add |
| starttime | u64/instant | anti PID-reuse |
| alive | bool | false no death |
| add_mode | pid\|exact\|substring | exact=`comm`/nome; substring=cmdline completo |

## Sample (RAM)
| campo | tipo | regra |
|---|---|---|
| ts | monotonic | crescente |
| pid | u32 | |
| rss_bytes | u64 | Linux VmRSS |
| cpu_pct | f32 | normalizado por núcleo; UI declara |

## Sample VRAM (placa) — só **agora** (último ponto)
| campo | tipo | regra |
|---|---|---|
| ts | monotonic | |
| gpu_index | u32 | MVP = 0 |
| total_bytes | u64 | |
| used_bytes | u64 | |
| free_bytes | u64 | |

## Sample VRAM (processo) — retido na **janela** (histórico)
| campo | tipo | regra |
|---|---|---|
| ts | monotonic | |
| pid | u32 | |
| used_bytes | u64 | 0 ou N/A se sem contexto; série na mesma window_secs da RAM |

## WindowConfig
| campo | valores |
|---|---|
| window_secs | 30 \| 300 \| 1800 \| 3600 |
| interval_ms | 100 \| 250 \| 500 \| 1000 \| 2000 |

Nulos: sem GPU → séries VRAM ausentes (não zerar mentindo). PID morto → última amostra fica; alive=false.
