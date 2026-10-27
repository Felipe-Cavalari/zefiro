# Chip Bandeja: os ícones da bandeja do sistema na barra — design

Data: 2026-10-26. Combinado em conversa; desenho A (grade, igual ao Windows) escolhido com os mockups.

## Intenção

O usuário usa a bandeja do Windows para **abrir** um app (clique esquerdo) ou **fechar** pelo menu
dele (clique direito → Sair). Quer o mesmo atalho na barra do Zefiro, lá em cima, perto de onde o
mouse já está, **guardado atrás de um botão, exatamente como no Windows**.

A barra de tarefas do Windows **continua visível** embaixo, com a bandeja dela funcionando. O
Zefiro não substitui nada: mostra os mesmos ícones também na barra. Isso segue a decisão de
2026-10-26 de complementar a barra de tarefas em vez de trocá-la (`docs/PLANNING.md`, Fase 4
removida).

## O chip

| id | Nome | Na barra | Clique |
| --- | --- | --- | --- |
| `tray` | Bandeja | Só uma seta para baixo (Segoe Fluent "ChevronDown", `E70D`), largura fixa como os chips de som e Terminal. | Abre o painel "Bandeja". |

Descrição no catálogo: "Ícones da bandeja do sistema; clique abre, botão direito mostra o menu do
app". Entra em qualquer seção de qualquer monitor pelo Editar barra, como os outros chips.

Como todos os chips, ele é livre: o Zefiro **não** o põe sozinho no config de ninguém. O usuário
escolhe onde ele fica pelo Editar barra (pedido do usuário).

## O painel

- Nasce do chip, como os outros (`PanelKind::Tray`). Sem título e sem lápis: só a grade, como o
  painel da setinha do Windows 11 (print do usuário).
- **Grade de 5 colunas**, ícones de 16 DIP em células de ~32 DIP, cantos e espaçamento do tema.
  Quantas linhas couberem; com muitos ícones, a grade rola com a roda (o painel não passa da
  altura do monitor).
- **Mostra todos os ícones da bandeja**, inclusive os que o Windows deixa fixos ao lado do relógio.
  Os que o app marcou como ocultos (`NIS_HIDDEN`) ficam de fora.
- **Ordem:** a de chegada (o primeiro `NIM_ADD` visto vem primeiro). Sem edição de ordem nem de
  ocultos nesta versão.
- **Hover:** a célula ganha o fundo de hover do tema e o **nome do app** aparece. Escolha minha: o
  Zefiro não tem janela de dica; o nome é desenhado **dentro do painel**, numa linha embaixo da
  grade, com a primeira linha do `szTip` (ex.: "Discord", e não "Discord\n3 mensagens"). Sem
  `szTip`, o nome do executável do processo dono.
- **Ícone que muda** (Discord com mensagem nova, OneDrive sincronizando) atualiza no painel aberto
  na hora, sem refazer o layout se a quantidade não mudou.
- **Clique esquerdo** num ícone: fecha o painel e manda o clique ao app.
- **Clique direito**: fecha o painel e manda o clique direito ao app; o app mostra o próprio menu
  (o mesmo da bandeja do Windows), onde fica o "Sair".
- Teclado: setas movem a seleção na grade, Enter = clique esquerdo, Shift+F10 ou a tecla de menu =
  clique direito, Esc fecha (como os outros painéis).
- Sem nenhum ícone: o texto "Nenhum ícone na bandeja" no lugar da grade.
- Balões (`NIF_INFO`) ficam de fora: o Windows já mostra esses avisos como notificação.

## De onde vêm os ícones

O Windows entrega cada ícone da bandeja à janela da barra de tarefas: `Shell_NotifyIcon` procura a
janela de classe `Shell_TrayWnd` (`FindWindow`) e manda os dados por `WM_COPYDATA`. Referências do
formato: notas do Geoff Chappell sobre o shell32, o `systray.c` do Wine e o commit "WM_COPYDATA for
Taskbar Interface" do ReactOS.

1. **Janela própria `Shell_TrayWnd`.** Com o chip em algum layout (`BarConfig::uses("tray")`), uma
   thread `zefiro-tray-host` cria uma janela de classe `Shell_TrayWnd`, invisível, sempre no topo e
   **acima** da barra de tarefas na ordem Z, para o `FindWindow` dos apps achá-la primeiro.
2. **Repasse de tudo.** Toda `WM_COPYDATA` que chega é repassada **sem alteração** à barra de
   tarefas de verdade (`SendMessageTimeout` com `SMTO_ABORTIFHUNG`, poucas centenas de ms), e a
   resposta dela é a resposta devolvida. Isso inclui as mensagens de AppBar (`SHAppBarMessage`, o
   próprio Zefiro é uma AppBar) e o ícone do próprio Zefiro (`tray.rs`). Só as de ícone de bandeja
   (`dwData = 1`) são lidas, além de repassadas.
3. **Pegar os ícones que já existem.** Logo depois de criar a janela, o host manda o
   `TaskbarCreated` (o mesmo aviso de quando o Explorer reinicia) e cada app manda o ícone de novo.
   Um `NIM_ADD` repetido na barra de tarefas de verdade falha sem efeito.
4. **Leitura.** `NIM_ADD`, `NIM_MODIFY`, `NIM_DELETE` e `NIM_SETVERSION` atualizam o modelo. A
   chave de um ícone é o `guidItem` quando vem `NIF_GUID`, senão o par (`hWnd`, `uID`). O `HICON`
   é de outro processo e pode sumir a qualquer hora: ele vira `Image` **na hora**, na thread do host
   (`DrawIconEx` num DIB, como `app_icon::hicon_to_image`), e o handle não é guardado.
5. **Ícones órfãos.** Um ícone cuja janela (`hWnd`) não existe mais sai do modelo (o app morreu sem
   `NIM_DELETE`). Conferido ao abrir o painel.
6. **Aviso à interface.** A thread do host só copia, converte e repassa; ela **nunca** espera a
   thread da interface. Mudanças vão por `post_user`, como os outros serviços.

### Clique

O clique é devolvido ao app como a barra de tarefas faz, na mensagem de callback dele
(`uCallbackMessage`, para o `hWnd` do ícone), conforme a versão que ele pediu (`NIM_SETVERSION`):

- **Versão 4:** `wParam` = posição do cursor (x, y), `lParam` = (mensagem, `uID`). Esquerdo:
  `WM_LBUTTONDOWN`, `WM_LBUTTONUP`, `NIN_SELECT`. Direito: `WM_RBUTTONDOWN`, `WM_RBUTTONUP`,
  `WM_CONTEXTMENU`.
- **Versões antigas:** `wParam` = `uID`, `lParam` = mensagem. Esquerdo: `WM_LBUTTONDOWN`,
  `WM_LBUTTONUP`. Direito: `WM_RBUTTONDOWN`, `WM_RBUTTONUP`.

Antes de mandar, o Zefiro chama `AllowSetForegroundWindow` com o processo do app, senão a janela ou
o menu do app abre atrás. O Zefiro tem esse direito porque o clique veio do usuário. As mensagens
vão por `PostMessage` (nunca `SendMessage`: um app travado não pode travar a barra).

### Robustez

- **Ordem Z.** Se a barra de tarefas subir acima da janela do host (o usuário clica nela), os apps
  passam a falar só com ela e o painel fica desatualizado. O host volta para cima a cada troca de
  foco (o `window::ForegroundHook` que já existe) e num temporizador leve.
- **Posição.** Alguns apps leem o retângulo da `Shell_TrayWnd` (e da filha `TrayNotifyWnd`) para
  posicionar a própria janela perto da bandeja. A janela do host tem **o mesmo retângulo** da
  barra de tarefas de verdade, e uma filha `TrayNotifyWnd` com o retângulo da área de notificação
  dela, atualizados quando a barra de tarefas muda.
- **Explorer reiniciando.** Quando a barra de tarefas de verdade some, o host destrói a própria
  janela (para o Explorer subir sem achar uma `Shell_TrayWnd` alheia), espera o `TaskbarCreated` e
  recria.
- **Chip saiu da barra / Zefiro fechou ou caiu.** A janela do host some e o `FindWindow` volta a
  achar a barra de tarefas de verdade sozinho. Nada fica preso.
- **Outras mensagens para a `Shell_TrayWnd`.** Com o host no topo, todo `FindWindow` acha ele, e
  não só o `Shell_NotifyIcon`: há apps que mandam `WM_COMMAND`/`WM_USER` à barra de tarefas
  (mostrar área de trabalho, minimizar tudo). Escolha minha: toda mensagem que **não leva
  ponteiro** é repassada como veio (`PostMessage` se veio postada, `SendMessageTimeout` se veio
  enviada), e o `WM_COPYDATA` como descrito acima. Mensagens com ponteiro além do `WM_COPYDATA`
  não são conhecidas; se o spike achar alguma, ela entra na lista. `IsWindowVisible` e o id do
  processo (o host é invisível e é do Zefiro, não do Explorer) ficam como limitação registrada no
  ADR, a não ser que o spike mostre um app comum quebrando por isso.
- **O `TaskbarCreated` falso.** Escolha minha: ele sai **uma vez por criação do host** (início do
  Zefiro com o chip, chip adicionado, Explorer reiniciado), nunca por temporizador. O próprio
  Zefiro trata esse aviso como "Explorer reiniciou" (`robustness.rs`: refaz as AppBars, repõe o
  ícone e a ilha); o host marca o aviso que ele mesmo mandou e o laço do Zefiro **ignora** esse.
  Outros apps de AppBar (YASB) e de bandeja reagem como a um reinício do Explorer; o spike observa
  se isso pisca ou mexe em algo visível, e se mexer o plano para.
- **Ícone do próprio Zefiro.** O `tray::add` roda na thread da interface e o host em outra: o
  `SendMessage` do `Shell_NotifyIcon` não trava, porque o host não espera a interface.

## O spike primeiro

Antes da interface, um spike descartável (`crates/platform-windows/examples/tray_host_probe.rs`)
responde neste PC (Windows 11 26200), com os apps do usuário abertos:

1. O repasse mantém a bandeja do Windows funcionando (ícones novos, mudança, remoção, o ícone do
   Zefiro, a reserva da AppBar)?
2. Depois do `TaskbarCreated`, quais apps mandam o ícone de novo e quais não?
3. O host continua primeiro na ordem Z depois de clicar na barra de tarefas?
4. O clique direito devolvido abre o menu do Discord e do Steam na frente?
5. O Explorer reiniciado (`Stop-Process explorer`, só com o "sim" do usuário) volta normal?

6. O `TaskbarCreated` falso faz algum app (YASB, Discord, Steam) piscar ou mexer em algo visível?
7. Algum app comum quebra por achar o host em vez da barra de tarefas (posição de janela perto da
   bandeja, "mostrar área de trabalho")?

Se 1, 4, 5 ou 6 falhar, a abordagem não serve e o plano para: abrir e fechar pelo menu do app é o
motivo do chip (escolha minha, a partir do que o usuário disse). Se 2, 3 ou 7 falharem em parte,
registrar a limitação no ADR e seguir.

## Onde mora cada parte

| Parte | Onde |
| --- | --- |
| Formato do `WM_COPYDATA` (bytes → mensagem), modelo dos ícones (add/modify/delete/versão, ordem, ocultos) e as mensagens de clique por versão | Crate nova, portável: `crates/integrations/systray` (testes no Linux) |
| Grade, hover, nome, rolagem, teclado e o que cada clique faz | `crates/modules/bar/src/systray.rs` |
| `PanelKind::Tray`, `panel_kind_for("tray")`, catálogo | `crates/modules/bar` |
| Janela `Shell_TrayWnd`, thread, repasse, ordem Z, `HICON` → `Image`, `PostMessage` do clique | `crates/platform-windows/src/tray_host.rs` (o `tray.rs` de hoje continua sendo o ícone do Zefiro) |
| Ligar e desligar o host com o chip, o painel, os cliques | `crates/app/src/systray_loop.rs` |
| Decisão | ADR novo em `docs/adr/` |

O nome `systray` separa a bandeja dos outros apps do `tray` que já existe (o ícone do próprio
Zefiro).

## Não objetivos

- Esconder ou substituir a barra de tarefas do Windows.
- Editar ordem ou esconder ícones no painel (pode vir depois, como no chip Terminal).
- Ícones fixos à vista na barra (fora do botão).
- Balões e avisos da bandeja.
- Duplo clique, clique do meio e arrastar para o ícone.

## Anti-objetivos

Falha mesmo funcionando se:

- a bandeja do Windows parar de mostrar algum ícone, ou o Zefiro deixar de reservar o espaço dele
  (AppBar), por causa do repasse;
- um app travar ou ficar lento ao pôr o ícone porque o host do Zefiro demorou;
- o Explorer não voltar ao reiniciar com o Zefiro aberto;
- a barra gastar CPU com o painel fechado (só o temporizador leve da ordem Z).

## Verificação

- Testes da crate `systray`: parser com bytes reais capturados no spike (fixtures), modelo (add,
  modify, delete, versão, chave por GUID, oculto), mensagens de clique por versão.
- Testes do painel: grade de 5 colunas, linhas, hover → nome, clique → ação certa, teclado,
  painel vazio.
- Sonda `tray_host_probe` (o spike vira sonda): lista os ícones vistos e confirma o repasse.
- Na tela, com o "sim" do usuário: abrir o Discord pelo painel, fechar o Steam pelo menu do clique
  direito, conferir a bandeja do Windows embaixo com os mesmos ícones, reiniciar o Explorer.
- `scripts/perf.ps1` com o chip: sem aumento perceptível de CPU em repouso.

## O que fica com quem constrói

Tamanhos exatos de célula e espaçamento (seguindo os tokens do tema), o intervalo do temporizador da
ordem Z, o timeout do repasse, como achar o retângulo da área de notificação da barra de tarefas de
verdade, e a estrutura exata dos bytes do `WM_COPYDATA` (confirmada no spike).
