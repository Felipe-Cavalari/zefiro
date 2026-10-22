# Gerenciador de janelas em mosaico (Fase 6 adiantada) — design

Data: 2026-10-21. Pedido: um gerenciador de janelas em mosaico no estilo do Hyprland, integrado ao
Zefiro, com a mesma tecnologia (Rust + Win32/Windows.UI.Composition, sem WebView) e com foco em
pouca RAM, pouco disco, animações suaves e rapidez. Referências estudadas: o komorebi (código lido
só para estudo; a licença dele, Komorebi License 2.0, não permite copiar código para outro projeto)
e o Hyprland (wiki).

O `docs/PLANNING.md` põe o gerenciador de janelas na Fase 6, condicionada a "só se a integração com
o komorebi não bastar". O usuário decidiu adiantar a fase e fazer um gerenciador próprio, sem passar
pelo komorebi. Isso vai num ADR novo (ADR-0021), que substitui essa condição.

Decisões do usuário estão marcadas como **(usuário)**; as do agente, como **(agente)**. O usuário
preferiu ver rodando antes de discutir o que o incomodaria no uso diário, então os detalhes de
gosto (atalhos além de Win+setas, espaços, molas) são padrões configuráveis, para ajustar depois.

## Intenção

Um complemento, não a cura de uma dor: o usuário tem 2 monitores e quer as janelas se organizando
sozinhas, com espaços entre elas e animações bonitas, como no Hyprland. O que mais importa, nesta
ordem **(usuário)**: o jeito como as janelas se dividem, os espaços e as animações.

## Metas

1. **Divisão dwindle** (árvore binária do Hyprland): cada janela nova divide a área da janela em
   foco; o sentido segue o lado mais longo da área (mais larga → lado a lado; mais alta → em cima e
   embaixo).
2. **Espaços**: entre janelas (`gaps_in`) e entre janelas e a borda da área útil do monitor
   (`gaps_out`), em DIP, convertidos pelo DPI de cada monitor.
3. **Áreas de trabalho por monitor**, como no Hyprland **(usuário)**: cada monitor troca as suas sem
   mexer no outro. São do Zefiro, independentes das áreas de trabalho do Windows **(usuário)**.
4. **Animações pelo compositor** (molas, CPU ~0 durante a animação): abrir, fechar, mover/
   redimensionar e trocar de área.
5. **Win+setas movem a janela em foco** **(usuário)**: troca de lugar com a vizinha na direção; na
   borda do monitor, passa para o monitor vizinho.
6. **Liga/desliga**: `[wm] enabled` no `config.toml` e uma chave "Janelas em mosaico" no menu do Z.
   Desligado, o Windows se comporta como sempre (inclusive Win+setas).
7. **Borda da janela em foco** com a cor de destaque do tema e cantos arredondados.
8. **Chip `workspaces`** na barra: mostra as áreas do monitor daquela barra, marca a ativa e as que
   têm janelas; clique troca de área.

## Não metas (desta versão)

- Outros layouts (master, colunas/rolagem, monocle), pilhas/abas de janelas, área especial
  ("scratchpad").
- Usar as áreas de trabalho do Windows ou conviver com elas (o usuário descartou a "unificação").
- Interface para regras por janela (as regras ficam no `config.toml`).
- O **app de configurações** estilo PowerToys, com a lista de features para ligar e desligar: é o
  próximo projeto, com brainstorm e spec próprios **(usuário: ordem combinada)**. Quando existir, a
  chave do menu do Z vira só um atalho.
- Conteúdo ao vivo durante a animação: a janela anima como uma imagem congelada (100–300 ms).
- Windows 10 (a borda colorida e os cantos usam atributos do DWM do Windows 11).

## Anti-metas (seria um fracasso mesmo funcionando)

- **Perder uma janela**: janela escondida que nunca volta (Zefiro fechado à força, travado, panic,
  monitor desligado). Ver "Segurança contra janelas perdidas".
- **Win+seta abrindo o menu Iniciar** ao soltar a tecla Win, ou teclado com atraso perceptível.
- **Briga com o app**: janela que recusa o tamanho e o Zefiro reaplicando em laço.
- **Mexer em janelas do próprio Zefiro** (barra, painéis, ilha, HUD) ou em janelas que não são de
  app (menus, dicas, Iniciar, Alt+Tab, notificações).
- **CPU por quadro** durante animação, ou consulta periódica (polling) em repouso.
- **Atrapalhar as sessões que mexem na barra**: o código fica em crates/módulos próprios; o toque na
  barra se limita ao chip `workspaces`, à chave do menu e ao `[wm]` no config.

## Restrições

- Regra de ouro da crate `windows` (AGENTS.md): a lógica portável (árvore, layout, áreas, regras,
  atalhos, planejamento de animação) numa crate nova **sem** `windows`, testável no Linux; o que fala
  com o Windows em `crates/platform-windows/src/wm/`; a ligação em `crates/app`.
- `unsafe` só em blocos pequenos com `// SAFETY:`; clippy `-D warnings`.
- Animação só no compositor (ADR-0001, ADR-0007; memória do projeto: nunca redesenhar por
  `WM_TIMER`).
- Desenho com o dispositivo WARP único (ADR-0001). Qualquer dispositivo de GPU extra precisa de um
  spike que meça a RAM e de um ADR.
- Nenhuma janela do Zefiro do tamanho exato do monitor (AGENTS.md): a camada das animações cobre a
  área útil, não o monitor inteiro.
- O usuário pode estar usando o PC: sondas que mandam teclas ou movem janelas pedem um momento antes.

## Metas de desempenho

| Métrica | Meta |
| --- | --- |
| Memória privada a mais com `[wm] enabled = true`, em repouso | ≤ 5 MB |
| CPU em repouso (WM ligado, nada mudando) | ~0 % (só eventos, nada de polling) |
| Tempo do evento (janela nova, Win+seta) até o `SetWindowPos` final | ≤ 16 ms (sem contar a animação) |
| CPU do Zefiro durante uma animação | ~0 % (molas no DWM) |
| Atraso do gancho de teclado por tecla | ≤ 1 ms (só compara e repassa) |
| Disco (binário) | ≤ 300 KB a mais no `zefiro.exe`, sem dependência nova pesada |

Medir com `perf.ps1` e `frames.ps1` como na Fase 1, com o YASB fechado (AGENTS.md, regra 4).

## Como funciona

### Peças e threads (agente)

```
crates/modules/wm            (zefiro-wm, portável)
  tree.rs        árvore dwindle: inserir, remover, trocar, vizinho por direção, razão das divisões
  layout.rs      árvore + área útil + gaps + DPI → retângulos por janela
  workspace.rs   monitores → áreas → árvore + janelas flutuantes; foco; mover entre áreas/monitores
  rules.rs       quem é gerenciado, flutua ou é ignorado (dados da janela → decisão)
  keys.rs        tabela de atalhos (config → ação), sem códigos de tecla do Windows
  anim.rs        plano de animação: lista de (janela, de, para, tipo) a partir de dois layouts
  state.rs       o "modelo": recebe eventos, devolve comandos (mover, esconder, mostrar, animar)

crates/platform-windows/src/wm/
  events.rs      SetWinEventHook (fora do processo) → eventos do modelo
  keys.rs        WH_KEYBOARD_LL numa thread própria; supressão do menu Iniciar
  window_info.rs estilo, classe, exe, dono, elevação, moldura invisível (DWMWA_EXTENDED_FRAME_BOUNDS)
  place.rs       DeferWindowPos/SetWindowPos assíncrono
  cloak.rs       esconder/mostrar janelas (ver "Esconder janelas")
  border.rs      DWMWA_BORDER_COLOR e DWMWA_WINDOW_CORNER_PREFERENCE
  capture.rs     "foto" de uma janela para a animação (decidida no spike S1)
  overlay.rs     camada da animação por monitor (Windows.UI.Composition)

crates/app        liga/desliga o WM pelo config, chave do menu do Z, chip workspaces
```

- **Modelo puro, efeitos fora:** `zefiro-wm` é uma máquina de estados: entra um evento
  (`WindowShown`, `WindowDestroyed`, `FocusChanged`, `MoveSizeEnded`, `Key(Action)`,
  `MonitorsChanged`, `ConfigChanged`), saem comandos (`Place`, `Cloak`, `Uncloak`, `Border`,
  `Animate`). Assim quase tudo é testado sem Windows, com TDD.
- **Thread `zefiro-wm`**: dona do gancho de eventos (que exige um laço de mensagens na thread que o
  instalou), do modelo e das chamadas de posicionamento. Ela tem o próprio `Comp` (fila do
  `DispatcherQueue` por thread, ADR-0001) para a camada de animação, então um redesenho da barra
  nunca atrasa uma animação. Custo previsto: ~2 MB (tabela do ADR-0001, "Compositor"); medir.
- **Thread `zefiro-wm-keys`**: só o gancho de teclado. Ela compara a tecla com a tabela de atalhos
  e manda a ação para a `zefiro-wm` por mensagem. O Windows remove sem aviso um gancho que passa do
  `LowLevelHooksTimeout`, então nada de trabalho nessa thread.
- **Avisar a barra:** o estado das áreas (por monitor: quantas, qual ativa, quais têm janelas) vai
  para a thread da interface por `post_user`, como os outros serviços, para o chip `workspaces`.

### Quem entra no mosaico (agente)

Uma janela é **gerenciada** quando tudo isto vale (verificado em `window_info.rs`, decidido em
`rules.rs`):

- janela de nível superior, visível, não "cloaked" pelo sistema, sem dono (`GW_OWNER`), com barra de
  título ou borda redimensionável (`WS_CAPTION` ou `WS_THICKFRAME`), sem `WS_EX_TOOLWINDOW` e sem
  `WS_EX_NOACTIVATE`;
- não é do processo do Zefiro;
- não é de um processo elevado (com o Zefiro sem admin, o Windows bloqueia `SetWindowPos` nela por
  UIPI): fica de fora, com um aviso no log uma vez por processo;
- não bate numa regra `ignore`.

**Flutuantes** (posição livre, por cima das gerenciadas, ainda pertencem a uma área): diálogos
(`WS_EX_DLGMODALFRAME` ou janelas com dono, que já ficam de fora pela regra do dono e seguem a
janela-mãe), janelas sem `WS_THICKFRAME` (tamanho fixo), janelas que batem numa regra `float` e
janelas que o usuário soltar no modo flutuante pelo atalho.

Lista padrão de regras (agente; o usuário edita): `ignore` para as classes do shell
(`Shell_TrayWnd`, `Progman`, `WorkerW`, `Windows.UI.Core.CoreWindow` de Iniciar/Pesquisa/Central de
ações, `XamlExplorerHostIslandWindow`, `TopLevelWindowForOverflowXamlIsland`) e `float` para
`Picture-in-Picture`/"Picture in picture" (título) e para `Taskmgr.exe` (elevado, já fica de fora).

```toml
[[wm.rules]]
exe = "Spotify.exe"     # qualquer combinação de exe, class, title (title = trecho)
action = "float"        # float | ignore
```

### Árvore e layout (dwindle)

- Cada área tem uma árvore binária: folhas são janelas; nós internos guardam o sentido da divisão e
  a razão (`0.1..=0.9`, padrão `0.5`).
- **Inserir:** a janela nova divide a folha da janela em foco naquela área; se não houver foco, a
  última janela inserida. O sentido segue o retângulo atual da folha (largura ≥ altura → lado a
  lado). A nova vai para a direita/embaixo (agente; Hyprland `force_split = 2`). O sentido fica
  gravado no nó (`preserve_split` sempre ligado, agente), para a árvore não "virar" ao redimensionar.
- **Remover:** a irmã ocupa o lugar do pai.
- **Layout:** área útil do monitor (`rcWork`, que já desconta a AppBar do Zefiro), menos `gaps_out`;
  cada divisão separa os filhos com `gaps_in`. Valores em DIP × escala do monitor, arredondados para
  pixel inteiro sem deixar vão de 1 px entre irmãs.
- **Moldura invisível:** no Windows 10/11 o `GetWindowRect` inclui uma borda invisível de
  redimensionamento (~7 px nas laterais e embaixo). O `place.rs` compensa pela diferença entre
  `GetWindowRect` e `DWMWA_EXTENDED_FRAME_BOUNDS`, para os espaços ficarem exatos.
- **Tamanho mínimo:** se a janela não aceitar o retângulo (o resultado do `SetWindowPos` difere), o
  Zefiro não reaplica. Ela fica sobreposta e o log registra uma linha. Sem laço (anti-meta).
- **Maximizar/minimizar pelo usuário:** maximizada ocupa a área útil por cima das outras até ser
  restaurada; minimizada sai da árvore e volta, ao restaurar, como janela nova na área ativa do
  monitor em que estiver.
- **Arrastar com o mouse** (`EVENT_SYSTEM_MOVESIZEEND`): se a janela foi movida, troca de lugar com
  a janela gerenciada sob o cursor (ou volta ao lugar, se não houver uma); se foi redimensionada,
  ajusta a razão da divisão daquela borda (agente; o builder pode adiar o ajuste da razão se ficar
  caro, voltando a janela ao lugar).

### Áreas de trabalho por monitor

- `workspaces = 5` por monitor (agente; configurável), numeradas. Todas existem sempre; o chip mostra
  as vazias apagadas.
- Trocar de área no monitor M: as janelas da área antiga são escondidas, as da nova aparecem, com a
  animação de deslizar. O outro monitor não muda.
- Mover a janela em foco para outra área: ela sai da árvore atual, é escondida e entra na árvore da
  área de destino.
- **Monitores:** identificados pelo `MonitorIdentity` que a barra já usa. Ao desconectar um monitor,
  as janelas de todas as áreas dele vão para a área ativa do principal (todas visíveis; nada fica
  escondido num monitor que não existe). Ao reconectar, não voltam sozinhas (agente).
- **Na partida** (ou ao ligar o WM): todas as janelas visíveis que entram no mosaico vão para a área
  1 do monitor em que estão, em ordem de Z (de baixo para cima).
- **Ao desligar o WM ou sair do Zefiro:** todas as janelas escondidas voltam a aparecer, as bordas
  voltam ao padrão do Windows e as janelas ficam onde estão.

### Casos de borda (agente)

- **Janela nova** entra na área ativa do monitor em que apareceu (pelo centro do retângulo).
- **Foco ao trocar de área:** a última janela focada daquela área; área vazia → o foco fica na área
  de trabalho do Windows (nada é ativado à força).
- **Win+seta sem vizinha e sem monitor naquele lado:** não faz nada (e a tecla é engolida, para o
  encaixe nativo não brigar com o mosaico).
- **Ir para o monitor vizinho:** a janela entra na área ativa dele, dividindo a janela focada ali.
- **Win+Ctrl+N com N maior que `workspaces`:** não faz nada.
- **A janela muda de monitor sozinha** (o app se move, Win+Shift+seta nativo com uma flutuante):
  ao fim do movimento, ela passa para a área ativa do monitor de destino.
- **Tela cheia** (a janela cobre o monitor inteiro, como um jogo ou vídeo em F11): sai do mosaico
  enquanto estiver assim e volta ao sair; nada é animado nesse monitor nesse meio tempo.
- **Falha na imagem** de uma janela: aquela janela se move sem animação; as outras animam.
- **UWP** (`ApplicationFrameWindow`): gerenciada como as outras; o "cloak" que o sistema põe nelas
  quando suspensas conta como "não visível", não como "fechada".

### Esconder janelas (agente; confirmar no spike S2)

O komorebi usa como padrão o "cloak" do shell (`IApplicationView::SetCloak` pelo
`IServiceProvider` do `ImmersiveShell`, API não documentada); `SW_HIDE` quebra apps Electron e
`SW_MINIMIZE` falha com trocas rápidas (documentação do komorebi). O `DwmSetWindowAttribute
(DWMWA_CLOAK)` só vale para janelas do próprio processo.

Decisão: **cloak do shell** como caminho principal, implementado do zero a partir da descrição das
interfaces (não do código do komorebi). Na partida, o `cloak.rs` testa a interface; se ela não
existir nesta versão do Windows (os GUIDs mudam entre builds), cai para **minimizar** e registra um
aviso. O spike S2 confirma na build 26200 que uma janela escondida assim some da tela, da barra de
tarefas e do Alt+Tab, e volta inteira.

### Segurança contra janelas perdidas (anti-meta número 1)

- Antes de esconder qualquer janela, o HWND vai para `%LOCALAPPDATA%\zefiro\wm-hidden.txt`; ao
  mostrar, sai. A gravação é atômica (arquivo temporário + renomear).
- Na partida do Zefiro, **antes de tudo**, toda janela listada que ainda existe é mostrada, e o
  arquivo é zerado.
- O panic hook do `zefiro-core` e o caminho de saída (Sair, `WM_CLOSE`, `WM_ENDSESSION`) mostram
  todas as janelas escondidas.
- `zefiro.exe --wm-restore` mostra tudo o que está no arquivo e sai. É a saída de emergência se o
  Zefiro foi morto à força e não for reaberto.

### Atalhos (Win+setas do usuário; o resto é padrão do agente, configurável)

`RegisterHotKey` não pega Win+setas (o shell já usa para encaixar janelas). Por isso o gancho é
`WH_KEYBOARD_LL`:

- Ele só engole a tecla quando o WM está ligado **e** a janela em primeiro plano é gerenciada. Com
  uma flutuante, ignorada ou da área de trabalho em foco, a tecla passa e o encaixe nativo do Windows
  funciona.
- **Menu Iniciar:** quando o gancho engole uma tecla com Win pressionada, ele injeta um toque de uma
  tecla neutra sem uso (`VK 0xE8`, por `SendInput`) antes do Win subir, para o shell não abrir o
  Iniciar. Os eventos injetados (`LLKHF_INJECTED`) passam direto pelo gancho.

| Ação | Padrão |
| --- | --- |
| Mover a janela em foco (trocar com a vizinha; na borda, ir para o monitor vizinho) | **Win+←/→/↑/↓ (usuário)** |
| Focar a vizinha na direção | Win+Alt+setas |
| Área anterior/seguinte no monitor do foco | Win+Ctrl+←/→ (no Windows, trocam a área de trabalho nativa; o Zefiro assume com o WM ligado) |
| Mover a janela em foco para a área anterior/seguinte (e ir junto) | Win+Ctrl+Shift+←/→ |
| Ir para a área N / mover a janela para a área N | Win+Ctrl+1…9 / Win+Ctrl+Shift+1…9 |
| Alternar flutuante | Win+Ctrl+F |
| Inverter o sentido da divisão da janela em foco | Win+Ctrl+S |

```toml
[wm.keys]
move_left = "win+left"     # uma entrada por ação; "" desliga o atalho
focus_left = "win+alt+left"
# ...
```

O `keys.rs` portável converte os textos em combinações (modificadores + tecla com nome portável); o
`platform-windows` mapeia nomes ↔ códigos virtuais.

### Animações: imagem congelada sobre uma camada do compositor (agente; spike S1)

O komorebi anima chamando `SetWindowPos` a 60 Hz numa thread por animação (o próprio projeto chama
de instável e com artefatos) ou move uma janela "fantasma" com uma miniatura do DWM, também quadro a
quadro. O Zefiro já anima tudo com molas do DWM; o desenho aqui segue isso:

1. O modelo calcula o layout novo e o `anim.rs` gera o plano: para cada janela afetada, `de`, `para`
   e o tipo (`Open`, `Close`, `Move`, `WorkspaceIn`, `WorkspaceOut`).
2. O `capture.rs` tira uma imagem de cada janela afetada, ainda no lugar antigo.
3. A camada do monitor (janela própria, `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE |
   WS_EX_TOPMOST`, sem entrada, do tamanho da área útil) recebe um `SpriteVisual` por janela, no
   retângulo `de`, com cantos arredondados.
4. As janelas reais são escondidas (cloak), vão para o retângulo final num único
   `BeginDeferWindowPos`/`EndDeferWindowPos` (`SWP_NOACTIVATE | SWP_NOZORDER |
   SWP_ASYNCWINDOWPOS`, para um app travado não travar a thread `zefiro-wm`) e ficam escondidas.
5. Os sprites vão de `de` para `para` com molas (`Offset` e `Size`/`Scale` no compositor). A imagem
   estica durante a mudança de tamanho, como no Hyprland.
6. No fim (`Comp::on_settled`), as janelas reais aparecem, os sprites somem com um fade curto
   (~80 ms) e a camada se esconde. Uma mudança no meio da animação apenas redireciona as molas a
   partir de onde estão.

Tipos:

- `Open`: a janela nova nasce a 85 % do tamanho, com opacidade 0, e cresce até o lugar (o `popin`
  do Hyprland); as outras se movem para abrir espaço.
- `Close`: a imagem tirada antes de fechar encolhe até 85 % e some. Se não der tempo de tirar a
  imagem (a janela já sumiu), só as outras se movem.
- `Move`: troca de lugar, Win+setas, arrastar, mudança de layout.
- Troca de área: a área que sai desliza para fora no sentido da troca e a que entra desliza para
  dentro (o `slide` do Hyprland), dentro da área útil do monitor.

Molas: `SpringParams` próprias em `zefiro-ui` (`WM_MOVE`, `WM_OPEN`, `WM_WORKSPACE`), ajustadas no
uso. `[wm] animations = false` desliga tudo, e o efeito de animação do Windows desligado também
desliga (como a barra já faz).

**Como tirar a imagem — o spike S1 decide**, nesta ordem de preferência:

1. `Windows.Graphics.Capture` de um único quadro (`CreateCaptureItemForWindow`,
   `IsBorderRequired = false`, `IsCursorCaptureEnabled = false`) sobre o dispositivo **WARP** já
   existente, copiado para uma `CompositionDrawingSurface`. Medir: tempo até a imagem (meta ≤ 16 ms
   por janela, em paralelo), RAM, se funciona com janela escondida, com app de GPU (Brave, VS Code)
   e com a borda amarela de captura desligada sem pedido de permissão.
2. Miniatura do DWM numa janela "fantasma" por janela (`DwmRegisterThumbnail`), com o retângulo de
   destino atualizado pelo `DwmUpdateThumbnailProperties` a cada quadro, a partir de uma curva de
   mola amostrada. Conteúdo ao vivo, mas com trabalho por quadro na CPU (parecido com o fantasma do
   komorebi).
3. Sem imagem: a janela real se move com um único `SetWindowPos` e a animação fica só na borda.

O resultado e os números vão no ADR-0021.

### Borda e cantos

- Janela gerenciada em foco: `DWMWA_BORDER_COLOR` com a cor de destaque do tema (a mesma do
  `accent` da barra); as outras: `DWMWA_COLOR_DEFAULT`. Cantos: `DWMWA_WINDOW_CORNER_PREFERENCE =
  DWMWCP_ROUND` nas gerenciadas. Custo zero: quem desenha é o DWM.
- Essa borda tem 1 px. Uma borda mais grossa (janela extra por janela, como no komorebi) fica para
  depois; se for preciso, o spike S1 mede se os próprios sprites da camada servem.
- Ao desligar o WM, as bordas e os cantos voltam ao padrão.

### Configuração

```toml
[wm]
enabled = false        # agente: desligado por padrão; a chave do menu do Z liga
workspaces = 5
gaps_in = 5            # DIP; padrões do Hyprland
gaps_out = 20
animations = true
focus_border = true

[wm.keys]              # ver "Atalhos"

[[wm.rules]]           # ver "Quem entra no mosaico"
```

- `WmConfig` no `zefiro-core`, com `#[serde(default)]` como as outras seções. A gravação da chave do
  menu usa o `toml_edit` que preserva comentários.
- Salvar o config com o WM ligado reaplica gaps, regras e atalhos na hora (mesmo caminho do
  `refresh_services()`).

### Barra

- **Chave "Janelas em mosaico"** no menu do Z, como a do "Acrílico".
- **Chip `workspaces`** no catálogo (`[bar.sections]`): uma bolinha ou número por área do monitor
  daquela barra; a ativa com a cor de destaque, as com janelas cheias, as vazias apagadas. Clique
  troca de área. Com o WM desligado, o chip some (o mesmo `BarConfig::uses` que liga os serviços
  passa a ligar o WM só se `enabled`). Largura fixa pelo número de áreas, para o layout não pular.

### Diagnóstico

- Log: uma linha por decisão relevante (janela adotada, flutuante, ignorada por elevação, recusou o
  tamanho), sem títulos de janela (podem ter dados pessoais); exe e classe podem.
- HUD (`Ctrl+Alt+Shift+D`): janelas gerenciadas por monitor e o último evento do WM.
- Sonda `wm_probe` (como as outras sondas): abre um app de teste, confere com `GetWindowRect` e
  `DWMWA_EXTENDED_FRAME_BOUNDS` que os retângulos batem com o layout e que os gaps têm o tamanho
  certo; outra confere que Win+→ não abre o Iniciar (`GetForegroundWindow` depois de soltar). Pedem
  um momento ao usuário antes (AGENTS.md, regra 6).

## Etapas sugeridas para o plano

1. **Spikes** (branch de spike, medições no ADR-0021): S1 imagem para a animação; S2 esconder pelo
   shell na build 26200 e recuperação; S3 gancho de teclado com Win+setas sem abrir o Iniciar.
2. **`zefiro-wm` portável**: árvore, layout, áreas, regras, atalhos, plano de animação, com TDD.
3. **Plataforma sem animação**: eventos, informações da janela, posicionamento com a moldura
   invisível, esconder/mostrar com o arquivo de recuperação, bordas, teclado. Mosaico funcionando com
   movimentos instantâneos.
4. **Animações**: camada, imagens, molas.
5. **Barra**: `[wm]` no config, chave do menu, chip `workspaces`, HUD.
6. **Fechamento**: medições, ADR-0021, atualização do `PLANNING.md` (Fase 6 adiantada, sem a
   condição do komorebi) e do `AGENTS.md` (estrutura e coisas não óbvias).

## O que fica com quem implementar

- Nomes internos, a divisão exata dos módulos e as mensagens entre as threads.
- Os valores das molas e da escala de `Open`/`Close`, ajustados vendo na tela.
- O caminho de captura, pelos números do S1.
- Adiar o ajuste da razão por arrastar a borda, se custar caro (registrar como ruling).
