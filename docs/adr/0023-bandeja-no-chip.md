# ADR-0023: Bandeja do sistema no chip `tray`

- **Status:** aceito
- **Data:** 2026-10-26
- **Spec:** [Chip Bandeja](../superpowers/specs/2026-10-26-bandeja-design.md)
- **Plano:** [2026-10-26-bandeja](../superpowers/plans/2026-10-26-bandeja.md)
- **Commits:** `f4c7dd1` (spike e capturas), `c0222c5` (crate `systray`), `a6f9dc1` (host), `5ad3d44` (catálogo), `e2f48ef` (grade), `b6aa7ef` (ligação no app)

## Contexto

O usuário abre e fecha apps pela bandeja do Windows (clique esquerdo abre, botão direito → Sair) e
quer o mesmo atalho na barra do Zefiro, guardado atrás de um botão, como a setinha do Windows 11. A
barra de tarefas do Windows continua visível: o Zefiro complementa, não substitui (a Fase 4, Dock,
saiu do plano pelo mesmo motivo).

O Windows não tem API para ler a bandeja de outro processo. O `Shell_NotifyIcon` de cada app procura a
janela de classe `Shell_TrayWnd` e manda os dados por `WM_COPYDATA`.

## Opções consideradas

1. **Janela `Shell_TrayWnd` própria, acima da barra de tarefas, repassando tudo** (a técnica de shells
   alternativos, do Wine e do ReactOS). O Zefiro recebe cada ícone em primeira mão e a barra de tarefas
   recebe a mesma mensagem logo depois.
2. **UI Automation na área de notificação do Explorer.** Dá nomes e "clicar", mas não dá as imagens dos
   ícones nem o menu de contexto do app de forma confiável, e depende do XAML da barra do Windows 11.
3. **`TB_GETBUTTON` no `ToolbarWindow32` antigo.** Não existe mais no Windows 11.

## Números medidos

Spike (`f4c7dd1`) neste PC, com os apps do usuário abertos:

| Pergunta | Resultado |
|---|---|
| A bandeja do Windows continua funcionando com o repasse | sim, 0 falhas de repasse; o usuário conferiu a bandeja inteira |
| Os ícones voltam depois do `TaskbarCreated` | 18 de 18 (apps respondem `NIM_DELETE` + `NIM_ADD`); apps Electron voltam sem dica |
| O host continua primeiro depois de clicar na barra de tarefas | sim, em todas as conferências |
| O menu do app abre na frente | sim (Steam, clique esquerdo e direito) |
| Reiniciar o Explorer | o host sai em 0,5 s, o Explorer volta normal, o host volta com o `TaskbarCreated` dele |
| Algo pisca com o `TaskbarCreated` falso | só o reinício do Explorer piscou |

Formato confirmado pelas capturas: 1484 bytes no Windows 11, os 964 do `NOTIFYICONDATA` de 32 bits
(handles de 32 bits, `uVersion` em 808, `guidItem` em 944) e depois o caminho do executável
(`WCHAR[260]`, com GUID de pasta conhecida), usado como nome quando o app não manda dica.

Ambiente: Windows 11 Pro 26200, 2 monitores, Explorer padrão com Windhawk.

## Decisão

Opção 1. Uma thread `zefiro-tray-host` (`platform-windows/src/tray_host.rs`) cria a `Shell_TrayWnd` só
quando o chip `tray` está em algum layout, lê os ícones (`zefiro-integration-systray`, portável) e
repassa à barra de tarefas `WM_COPYDATA`, `WM_COMMAND`, `WM_USER..0x7FFF` e mensagens registradas,
nunca o `TaskbarCreated`. O clique volta ao app pela mensagem de callback dele, por `PostMessage`,
depois de `AllowSetForegroundWindow`.

## Consequências

- **`TaskbarCreated` marcado:** o host pede os ícones com `lParam = OWN_BROADCAST`. O Zefiro trata
  `WinEvent::TaskbarCreated { own: true }` como nada; sem isso ele refazia as AppBars 4 vezes (uma por
  janela de topo) a cada pedido.
- **Explorer reiniciando:** um temporizador de 1 s confere a barra de tarefas; quando ela some, o host
  sai da frente para o Explorer subir sem achar uma `Shell_TrayWnd` alheia, e volta no `TaskbarCreated`
  de verdade.
- **Limitações:** `IsWindowVisible` e o processo dono da primeira `Shell_TrayWnd` mudam enquanto o chip
  existe (o host é invisível e é do Zefiro); nenhum app comum quebrou no spike. Balões (`NIF_INFO`)
  ficam de fora. Shift+F10 não chega ao painel (é `WM_SYSKEYDOWN`); a tecla de menu abre o menu do app.
- **Nomes:** `tray.rs` é o ícone do próprio Zefiro; `tray_host.rs`, `systray` e `systray_loop.rs` são a
  bandeja dos outros apps.
- **Plano B:** se uma atualização do Windows mudar o formato, `wire::parse` devolve `None` e o painel
  fica vazio, sem afetar a bandeja do Windows (o repasse não depende da leitura).
