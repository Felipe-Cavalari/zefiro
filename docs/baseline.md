# Medição de referência (B0)

- **Data:** 2026-10-15
- **Ambiente:** Windows 11 Pro 26200, RTX 5070, 2× 1920×1080 (165 Hz + 144 Hz)
- **Configuração:** YASB v2.0.7, tema CavalariDev, 2 barras, **sem komorebi** (estava offline no uso diário)
- **Ferramenta:** `scripts/perf.ps1 -Process yasb -WarmupSeconds 60 -SampleSeconds 300` (soma processos filhos)

| Métrica | Execução 1 | Execução 2 | Execução 3 | Mediana |
|---|---|---|---|---|
| Working set (MB) | 164,4 | 175,5 | 176,2 | **175,5** |
| Memória privada (MB) | 101,1 | 105,7 | 105,9 | **105,7** |
| CPU média em repouso (%) | 0,474 | 0,119 | 0,186 | **0,186** |
| Tempo até a barra aparecer (s) | 2,62 | 2,11 | 2,22 | **2,22** |

Observações:
- A execução 1 rodou enquanto o S1 compilava em paralelo (CPU do YASB um pouco mais alta). As execuções 2 e 3 foram repetidas depois, porque o YASB fechou durante a primeira tentativa.
- O YASB fecha sozinho quando outra AppBar se registra ou o Explorer reinicia (aconteceu duas vezes durante os testes do S1).
- Seelen-UI: não medido (não instalado).

Metas do Zefiro (PLANNING §1, revistas no ADR-0001): memória privada ≤ 25 MB, working set acompanhado, CPU ≤ 0,3 %, primeiro quadro ≤ 0,25 s. Referência atual do spike S1 com WARP: 10,7 MB privada / 35 MB working set / ~0 % CPU / 338 ms.
