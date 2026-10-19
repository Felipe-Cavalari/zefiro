# ADR-0007: A barra da Fase 1a no design da ilha

- **Status:** aceito
- **Data:** 2026-10-15
- **Branch:** `fase-1a/barra`. Spec: [`2026-10-15-fase-1a-barra-design.md`](../superpowers/specs/2026-10-15-fase-1a-barra-design.md); plano: [`2026-10-15-fase-1a-barra.md`](../superpowers/plans/2026-10-15-fase-1a-barra.md).

## Contexto

A Fase 1a é a barra do Zefiro em Rust, feita do zero com o visual e o movimento do Dynamic Island for Windows. O pedido:

- duas alturas: completa (44 DIP) e compacta (32 DIP); **ajuste (2026-10-19):** a completa passou a 36 DIP, com chips de 28 e a pílula da ilha de 30, porque a 44 o usuário achou grande demais e a ilha (44, com contorno e sombra) parecia maior que a faixa. A ilha nunca passa de faixa − 4 DIP;
- dois formatos: faixa (ponta a ponta) e pílulas;
- tema Obsidian como padrão, mais os outros 7 temas da ilha, todos editáveis;
- painéis que abrem e fecham como a ilha (a pílula vira o painel);
- relógio e calendário;
- a ilha ([ADR-0006](0006-ilha-de-midia-fork.md)) embutida no centro.

O komorebi fica de fora até a 1.0. O pipeline de desenho segue o [ADR-0001](0001-pipeline-de-renderizacao.md): Windows.UI.Composition, Direct2D num dispositivo WARP e molas no compositor.

Este ADR registra as decisões tomadas durante a implementação, os números medidos e o que ficou diferente da spec.

## Decisões

1. **Listas de desenho portáveis, desenho na plataforma.** As crates portáveis (`zefiro-ui`, `zefiro-bar`, `zefiro-resources`) produzem dados: retângulos de layout, `DisplayList`s e tokens de tema. Só a `zefiro-platform-windows` desenha (Direct2D/DirectWrite em superfícies do compositor) e anima. A crate `windows` aparece apenas em `platform-windows`, em `integrations/clock/windows.rs` e em `app`, e o `scripts/check-windows-imports.ps1` cobra isso.

2. **Pílula → painel com escalares e expressões.** Cada forma (faixa, grupo de pílulas, fundo de chip, painel) é uma `MorphShape`:
   - um `CompositionPropertySet` com `x`, `y`, `w`, `h` e `r`, animados por `SpringScalarNaturalMotionAnimation`;
   - `ExpressionAnimation`s que ligam a esses escalares a geometria, o contorno, a sombra, o recorte do conteúdo e a opacidade do conteúdo. A opacidade segue a regra da ilha: o conteúdo aparece nos últimos 40% da altura.

   O conteúdo do painel é desenhado **uma vez**, já no lugar final; a forma cresce do chip até o painel. Abrir usa a mola de crescer (380/26) mais uma "cutucada" de −6 DIP; fechar usa a mola de encolher (200/28) e, no fim, um fade de 0,17 s, para a forma não ficar parada sobre o chip até a mola assentar.

3. **Uma janela de painel por monitor, reaproveitada e do tamanho do monitor.** Como a janela de desenho deixa todo clique passar (decisão 4), cobrir o monitor inteiro não custa nada. A forma vive sempre no mesmo espaço, então trocar de painel no meio de uma animação (menu → calendário) só redireciona a mola, sem salto.

   **Ajuste (2026-10-19):** a janela tem uma linha a menos que o monitor (`panel_window`). Com o tamanho exato do monitor e em primeiro plano, o Windows a tratava como app em tela cheia e, com a regra automática "Não perturbe em tela cheia" ligada, ligava o Não perturbe a cada painel aberto e desligava ao fechar. A ilha mostrava "Não perturbe desligado" toda hora. Nenhum painel chega à última linha da tela, então o desenho não muda.

4. **Duas janelas por barra e por painel.**
   - A janela de desenho é `WS_EX_LAYERED | WS_EX_TRANSPARENT`, então nenhum clique para nela.
   - Uma janela de entrada invisível (`ZefiroInput`), dona-dependente da de desenho, é recortada por `SetWindowRgn` nos retângulos clicáveis. Os eventos dela chegam com o `hwnd` da janela de desenho.

   O motivo: um hit-test `HTTRANSPARENT` só repassa o clique para janelas **da mesma thread**. Uma sonda com `WindowFromPoint` mostrou que a margem de sombra abaixo da faixa (24 DIP) engolia cliques nas janelas dos outros programas, como as abas e a barra de título de janelas maximizadas.

5. **Tema da ilha por dois arquivos.** `island::sync` grava:
   - no `island.ini`, a seção `[Zefiro]` (âncora, altura da barra e altura da pílula), `ThemePreset` com as cores em hex, e as animações;
   - no `island-state.ini`, `[Values] ColorThemeV2` (o índice do tema), que na ilha vence o `.ini`.

   Grava antes de iniciar a ilha e a cada mudança, só quando o texto muda. A ilha relê o `island-state.ini` sozinha.

   **Calendário só na barra:** a ilha embutida não tem mais a aba de calendário (`kCalendarTabs = 0` no fork). O calendário da barra segue o desenho do `DrawCalendarDashboard` da ilha:
   - tamanho fixo de 352×184;
   - o dia em 63 px, em negrito;
   - iniciais discretas e uma linha fina embaixo delas;
   - fim de semana recuado;
   - hoje como o único destaque, na cor da ilha.

   Foi um pedido do usuário depois da validação.

   No formato **faixa**, a ilha se funde à barra: `ContourBorderMode=borderless`, `DropShadow=0` e `AccentBloom=0` (sem o tom da capa no fundo). Foi um pedido do usuário depois da validação ("muito destoada"). No formato **pílulas**, ela continua uma pílula como as outras, com contorno, sombra e o tom da capa.

6. **Ilha em processo separado, com política de reinício.** A ilha roda numa thread que verifica a saída a cada 1 s. Saindo sozinha, reinicia até 3 vezes em 60 s; depois desiste, deixa o centro vazio e registra no log. **Reiniciar a ilha**, no menu, zera a política.

7. **O menu continua aberto ao trocar altura, formato ou tema.** A barra anima na hora e o menu é redesenhado com a nova marcação, acompanhando o chip (mola de ajuste) quando a barra muda de altura. Editar o `config.toml` à mão fecha o painel aberto, como manda a spec.

8. **Molas no compositor: `Period = 1/ω`.** A conversão da spec (ω = √k, período = 2π/ω, ζ = c/2ω) deixava as molas 4–5× mais lentas que as da ilha. Medido com quadros cronometrados: com 2π/ω, o GROW estava em ~52% aos ~480 ms, o que dá ω efetivo de ~3–4,5 em vez de 19,5. Com `Period = 1/ω`, o painel abre em ~170 ms, com o leve pulo da ilha. Ou seja: o `Period` do Windows.UI.Composition se comporta como 1/ω.

## Números medidos

Ambiente:

- Windows 11 Pro 26200, RTX 5070, 16 processadores lógicos.
- Dois monitores 1920×1080 (primário a 165 Hz).
- Medido em 2026-10-15, build release.
- YASB fechado. A ilha foi desligada (`[island] enabled = false`) para medir só a barra; os números da ilha estão no ADR-0006.

| Métrica | Medido | Meta (spec §13) |
|---|---|---|
| Clique → painel pronto para o primeiro quadro (50 ciclos, `ZEFIRO_CYCLES=50`) | p50 **2,1 ms**, p95 2,9 ms, máx. 15,1 ms (o primeiro, a frio) | ≤ 16 ms |
| Primeiro quadro da barra (log, 5 aberturas, 2 monitores) | **43–53 ms** (mediana 51) | ≤ 250 ms |
| Memória privada, repouso (MB) | **15,0** (working set 37,3) | ≤ 15 |
| CPU, repouso (%) | **0,0** | ≤ 0,05 |
| Memória privada, abrindo o calendário a cada ~5 s (MB) | 16,1 (working set 38,2) | acompanhar |
| CPU, abrindo o calendário a cada ~5 s (%) | < 0,01 (média arredondada a 0,0) | acompanhar |
| Quadros perdidos durante as animações (PresentMon) | **não medido** (ver abaixo) | ≈ 0% de intervalos > 1,5 período |

Arquivos: `scripts/out/zefiro-repouso.json` e `scripts/out/zefiro-paineis.json` (`perf.ps1`, 3 min de amostras; no segundo, o calendário abriu e fechou 40 vezes por cliques automáticos). A memória em repouso fica exatamente no limite: qualquer item novo na 1b–1f precisa ser medido contra ela. A CPU quase zero com painéis abrindo confirma o ADR-0001: a animação roda no compositor, fora do processo.

**Quadros perdidos:**

- O PresentMon precisa de elevação, e esta sessão não estava elevada: saiu com código 6.
- O plano B, os contadores de `DwmGetCompositionTimingInfo`, não andam neste Windows: 1 quadro contado em 50 s de animação.
- Fica para rodar num PowerShell elevado:

  ```powershell
  $env:ZEFIRO_CYCLES = 50
  ./target/release/zefiro.exe
  PresentMon --process_name dwm.exe --output_file scripts/out/zefiro-ciclos.csv --timed 50 --terminate_after_timed
  ./scripts/frames.ps1 -Csv scripts/out/zefiro-ciclos.csv -RefreshHz 165 -IdleGapMs 100
  ```

## O que ficou de fora ou divergiu

- **Cor de destaque `auto`:** vem da ilha. A ilha publica o destaque que está usando (a cor da capa do álbum tocando ou, sem mídia, o destaque dela) em `island-state.ini` como `[Values] ZefiroAccent=0xRRGGBB` (em decimal, porque o shim só guarda inteiros), só quando muda. A barra observa o arquivo e usa esse valor quando o tema diz `accent = "auto"`. Sem a ilha, a barra usa o destaque do Windows.
- **Ajustes de API da `windows 0.61.3`** feitos durante a implementação:
  - `LOCALE_NAME_USER_DEFAULT` não existe na crate; virou uma constante local `PCWSTR::null()`.
  - `ID2D1DeviceContext::CreateBitmap2` se chama `CreateBitmap`.
  - `WM_MOUSELEAVE` fica em `Win32::UI::Controls`; está definida localmente.
  - `CreateMutexW` exige a feature `Win32_Security`.
  - O `TypedEventHandler` exige `Send`, então `Comp::on_settled` guarda o callback num `Mutex`.
- **Sombra:** um `DropShadow` num `LayerVisual` deixava a janela inteira preta (provado por bisseção). A sombra virou a técnica da própria ilha (`DrawSoftShadow`): 7 retângulos arredondados concêntricos, com alfa caindo ao quadrado, ligados às mesmas expressões da forma. O espalhamento é clamp(14 × escala, 6, 18).
- **Painel cobrindo o monitor e janelas de entrada recortadas:** no lugar do `window_bounds` e do hit-test `HTTRANSPARENT` do plano (decisões 3 e 4).
- **Plano B da Task 18 (ilha):** não foi usado. A pílula recolhida da ilha encolhe com a barra (`PillHeight`, `CollapsedScale`) dentro do próprio fork.
- **Arquivos da barra antiga do Codex** em `%APPDATA%\zefiro\` (`config.toml` e `themes/cavalaridev.toml` e `light.toml`, em outro formato) foram **movidos**, não apagados, para `%APPDATA%\zefiro\codex-backup\`.
- **Não verificado por mim, fica para a validação do usuário:**
  - reiniciar o Explorer de verdade (havia janelas do Explorador abertas);
  - Win+P;
  - desligar os "Efeitos de animação" do Windows.

  Os caminhos de código foram exercitados postando `TaskbarCreated` e `WM_DISPLAYCHANGE` na barra: uma recriação só (debounce de 400 ms) e nenhuma janela vazada.
- **Achado na ilha (upstream):** a janela da ilha só fica click-through quando está Idle (`SetClickThrough`). Mostrando mídia, a margem dela, por volta de y 44–100 no centro, engole cliques nas janelas de baixo. É o mesmo problema que a barra tinha (decisão 4).

## Validação com o usuário

Em 2026-10-15, o usuário viu a galeria com as 4 combinações (completa/faixa, completa/pílulas, compacta/faixa, compacta/pílulas) em Obsidian e em Porcelain, mais o menu e o calendário abertos nos dois temas. Ele **aprovou tudo como está**, sem pedir mudanças.

Continuam pendentes, para ele rodar na própria máquina:

- reiniciar o Explorer de verdade;
- Win+P;
- desligar os "Efeitos de animação";
- a medição de quadros com o PresentMon elevado.

## Consequências

- A 1c (porte da ilha para Rust) herda a `MorphShape`, as janelas de entrada recortadas e a conversão de mola corrigida. A ilha em Rust não terá o problema do clique na margem.
- O PLANNING foi atualizado (§2, §3.5, §4.1, §6, §12): komorebi fora até a 1.0; a 1a é a spec da barra; a 1c é o porte da ilha.
- Riscos que sobram:
  - quadros perdidos não medidos com o PresentMon;
  - o caminho real de Explorer reiniciado e de troca de monitor ainda não testado na máquina.
