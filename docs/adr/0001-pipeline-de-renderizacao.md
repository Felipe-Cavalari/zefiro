# ADR-0001: Pipeline de renderização (compositor, acrílico, animação e dispositivo de desenho)

- **Status:** aceito
- **Data:** 2026-10-15
- **Spike/branch:** `spike/s1-render` (commits `ea7124d`…`19e011e` + instrumentação de etapas)

## Contexto

O S1 (spec da Fase 0, §8) precisava decidir a base visual de toda a barra: **DirectComposition puro (A)** ou **Windows.UI.Composition (B)**, a forma do acrílico e o método de animação, medindo RAM, CPU, primeiro quadro, clique→quadro e quadros perdidos.

## Opções consideradas

1. **A — DirectComposition**: superfícies `IDCompositionSurface`, acrílico do DWM na janela inteira (a1: `DWMSBT_TRANSIENTWINDOW`; a2: `SetWindowCompositionAttribute`, o mesmo do Dynamic Island), molas amostradas em curvas `IDCompositionAnimation`.
2. **B — Windows.UI.Composition**: `CompositionDrawingSurface` com Direct2D, `HostBackdropBrush` recortado por `CompositionGeometricClip` arredondado, molas nativas `SpringScalarNaturalMotionAnimation`.
3. Em cada caminho: animação **no compositor** × **quadro a quadro na thread de UI** (método do Dynamic Island).
4. Descoberto durante a medição: dispositivo de desenho **D3D11 na GPU** × **WARP** (rasterizador de software do Windows; a composição e as animações continuam na GPU, dentro do DWM).

## Números medidos

Ambiente: Windows 11 Pro 26200, RTX 5070, `DISPLAY2` primário 1920×1080 a **165 Hz**, `DISPLAY1` 1920×1080 a 144 Hz, YASB fechado, 2026-10-15. Medição automática: `spike/s1-render:spikes/s1-render/measure.ps1` (elevado, por causa do PresentMon).

### Com o dispositivo D3D11 na GPU (5 combinações)

| Métrica | B host, compositor | B host, quadro | A a1, compositor | A a1, quadro | A a2, compositor | Meta |
|---|---|---|---|---|---|---|
| Working set em repouso (MB) | 62,3 | 56,3 | 56,1 | 55,8 | 56,5 | ≤ 25 |
| Memória privada (MB) | 59,3 | 58,4 | 58,7 | 59,4 | 58,9 | — |
| CPU em repouso (%) | ~0 | ~0 | ~0 | ~0 | ~0 | ≤ 0,2 |
| CPU durante 50 ciclos (%) | ~0 | ~0 | ~0 | ~0 | ~0 | menor é melhor |
| Primeiro quadro (ms, 4 aberturas) | 408–426 | 404–450 | 449–458 | 426–540 | 426–596 | ≤ 250 |
| Clique→quadro (ms) | 2,0–9,8 | — | 2,7–9,2 | — | 8,7–11,0 | ≤ 16 |
| Intervalos > 1,5 período nas rajadas, monitor animado (165 Hz) | 0,96 % | 2,63 % | **0,12 %** | 0,52 % | 1,10 % | — |
| Idem, monitor sem animação (144 Hz, ruído de fundo) | 2,10 % | 5,46 % | 1,74 % | 4,92 % | 1,82 % | — |
| Acrílico recortado sem "caixa" | sim (recorte do visual) | sim | não (janela inteira) | não | não | sim |

"~0" significa abaixo da resolução da medição: o spike acumulou 0,39 s de CPU em ~2 min (≈ 0,02 % da máquina).

### Onde vão a RAM e o tempo (caminho B, etapas da inicialização)

| Etapa | GPU: tempo | GPU: working set | **WARP**: tempo | **WARP**: working set |
|---|---|---|---|---|
| Processo vazio | 0 | 10,0 MB | 0 | 10,0 MB |
| D3D11 | +187 ms | +24,3 MB (privada +48 MB) | +12 ms | +3,4 MB (privada +0,9 MB) |
| Direct2D + DirectWrite + fontes | +13 ms | +2,6 MB | +12 ms | +1,4 MB |
| Compositor | +7 ms | +1,9 MB | +7 ms | +2,0 MB |
| Barra 1: janela / AppBar / acrílico / camada / desenho | 6 / 11 / **39** / 9 / 13 ms | +2,9 MB | 7 / 12 / **35** / 9 / 13 ms | +2,8 MB |
| Barra 2: janela / AppBar / acrílico / camada / desenho | 31 / **56** / **44** / 9 / 13 ms | ~0 | 31 / **53** / **43** / 9 / 13 ms | ~0 |
| Painel + primeiro quadro | +43 ms | ~0 | +45 ms | ~0 |
| **Total até o primeiro quadro** | **520 ms** | **41,9 MB** | **338 ms** | **19,9 MB** |

Em repouso por 3 min com WARP (painel aberto e fechado): **working set 35,4 MB, memória privada 10,7 MB, CPU ~0 %**. O que sobra acima da memória privada são páginas compartilhadas de DLLs do sistema (WARP, DirectWrite, Windows.UI.Composition). O usuário confirmou que o desenho com WARP fica idêntico ao da GPU.

### Validações funcionais

- **AppBar:** reserva o espaço nos dois monitores; convive com outra AppBar (fica abaixo do YASB). O painel passou a se posicionar pela barra real (bug encontrado com o YASB aberto, corrigido com teste).
- **Win+P** (somente PC → estender): barras recriadas 2 → 1 → 2 sem fechar. Cada mudança chega como até **3 `WM_DISPLAYCHANGE`**.
- **Reinício do Explorer:** `TaskbarCreated` recebido, AppBars registradas de novo.
- **Driver de vídeo reiniciado (`Win+Ctrl+Shift+B`):** chega como 3 `WM_DISPLAYCHANGE` (não como dispositivo perdido). Revelou um bug do caminho B: recriar o app criava uma segunda `DispatcherQueueController` na mesma thread e o processo fechava. Corrigido (uma fila por thread) com teste de regressão.
- **Mudança de escala (DPI):** não testável neste PC (desloga a sessão do usuário). Fica em aberto.
- **Windows 10:** não testado (não há VM pronta). Fica em aberto.

## Decisão

**Caminho B (Windows.UI.Composition)**, com:
- acrílico via **`HostBackdropBrush` recortado pela geometria do visual**;
- **molas no compositor** (`SpringScalarNaturalMotionAnimation`), não quadro a quadro;
- conteúdo desenhado com **Direct2D sobre um dispositivo D3D11 WARP**, não na GPU.

Por quê:
1. Nenhuma combinação com a GPU cumpre a meta de RAM; a causa é o dispositivo D3D11 do driver (+24 MB, +48 MB de memória privada), igual nos dois caminhos. Com o WARP o custo some, e como a barra só redesenha quando um dado muda, o desenho por software é barato (CPU ~0 %).
2. Pela regra de desempate da spec, o **acrílico recortado pela forma** decide: só o caminho B o tem, e ele é essencial para a ilha de mídia (S6).
3. Animar no compositor foi melhor que quadro a quadro nos dois caminhos e deixa a CPU do Zefiro em ~0 % durante a animação.

**Metas que não foram cumpridas (decisão consciente):**
- **Primeiro quadro: 338 ms × meta de 250 ms.** O tempo restante está em chamadas síncronas ao sistema: acrílico (~40 ms por janela) e registro da AppBar (até ~55 ms). Plano da Fase 1: mostrar as barras e desenhar antes de registrar a AppBar e aplicar o acrílico (em paralelo por janela), e medir de novo. A meta continua 250 ms.
- **RAM: meta de 25 MB redefinida como memória privada ≤ 25 MB** (medido: 10,7 MB). O working set (35 MB) inclui páginas de DLLs do sistema divididas entre processos, que o app não controla; acompanhar os dois números no `perf.ps1`.

## Consequências

- `zefiro-platform-windows` cria **um único** dispositivo D3D11 WARP para todo o desenho (Direct2D); a GPU fica só com o DWM. Se algum widget futuro precisar de desenho pesado (ex.: blur próprio, vídeo), reavaliar com um dispositivo de GPU só para ele.
- **Uma `DispatcherQueueController` por thread**, reaproveitada em reconstruções.
- Reconstruções por mudança de monitor precisam de **debounce** (vários `WM_DISPLAYCHANGE` por evento).
- O painel e qualquer janela ancorada se posicionam pela **posição real da barra**, nunca pelo topo do monitor.
- **Quadros perdidos por app** não são mensuráveis de forma limpa pelo DWM num desktop ocupado (o monitor sem animação já mostra 1,7–5,5 % de intervalos longos). O S6 mede com um contador dentro do próprio app (`FrameWatch`).
- Riscos em aberto: DPI 125 % (não testável aqui), Windows 10, e se o `HostBackdropBrush` funciona sem identidade de pacote em todas as máquinas (funcionou aqui sem pacote).
