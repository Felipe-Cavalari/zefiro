# ADR-0022: Barra limpa (ocultar automaticamente, só no principal) e Configurações

- **Status:** aceito
- **Data:** 2026-10-22
- **Spec:** [Barra limpa](../superpowers/specs/2026-10-22-barra-limpa-design.md)
- **Commits:** `cd43767` (opções e só no principal), `783510b` (ocultar automaticamente), `93520b1` (menu e Configurações)

## Contexto

Um usuário do Zefiro pediu um setup mais limpo: a barra fora do caminho até ele precisar dela, e só no
monitor principal. Junto com isso, o menu do Z tinha crescido com itens raros (iniciar com o Windows,
HUD, diagnóstico, reiniciar a ilha) e com a chave do mosaico, que ainda não vai para os usuários.

## Opções consideradas

**Onde está o cursor (ocultar automaticamente):**

1. Uma janela clicável de 1 px no topo. Simples, mas come o clique da linha de cima: aba do navegador e
   barra de título maximizadas ficam colados no topo. Rejeitada.
2. `GetCursorPos` num temporizador. Custa CPU em repouso e contraria "nada consulta por quadro".
   Rejeitada.
3. **O hook `WH_MOUSE_LL` que já existe (`bar_context`, botão direito na faixa vazia)** também observa
   `WM_MOUSEMOVE`, compara com o topo e a faixa de cada barra e só acorda o laço quando algo muda
   (`WM_APP + 5` → `WinEvent::AutoHideCursor`). Escolhida: nenhum clique é retido e sem a opção não há
   trabalho novo (a lista de observação fica vazia).

**Reserva de espaço:**

1. `ABM_SETAUTOHIDEBAREX`. Pensado para a barra de tarefas, briga com ela pela borda e não dá controle da
   animação.
2. **Sem AppBar com a opção ligada.** Escolhida: as janelas maximizadas vão até o topo, e a barra é uma
   janela sempre-no-topo comum. A tela cheia continua detectada pelo gancho de foco e pelo temporizador
   (`reconcile_fullscreen`, da correção dos jogos); o `ABN_FULLSCREENAPP` deixa de chegar e não faz falta.

**Nome da chave do monitor:** `[bar] monitors` já são os layouts próprios (`[bar.monitors.*]`), e as duas
coisas não cabem no mesmo TOML. Ficou `[bar] display = "todos" | "principal"`.

## Decisão

- A regra é portável (`zefiro_ui::autohide`): desce depois de **300 ms** com o cursor num gatilho do topo
  (`top_triggers`; ver "Topo coberto por outro monitor", abaixo), sobe **500 ms** depois de o cursor sair
  da faixa, fica descida com painel, menu ou Editar barra abertos no monitor, e fica fora com tela cheia.
  Avisos da ilha não a trazem. A regra roda depois de cada evento do laço; os prazos usam
  `TIMER_AUTOHIDE`.
- A barra desliza com mola no `Offset` do visual raiz (`BarView::set_retracted`); a janela some quando a
  mola assenta (`on_settled`, com geração para não esconder depois de uma descida nova), levando junto as
  áreas de clique e a zona do botão direito. Escondida por tela cheia, ela recolhe sem animar, para não
  piscar quando o jogo fecha.
- Com a ilha externa a opção é ignorada (a ilha C++ é outro processo e não acompanharia a barra); o
  Editar barra mostra a chave apagada.
- `display = "principal"` cria barra e painel só no monitor principal (`BarConfig::monitors_with_bar`,
  com o primeiro monitor como reserva se nenhum for principal). Trocar recria as barras pelo caminho do
  `TIMER_DISPLAY`. Os layouts dos outros monitores ficam no arquivo.
- As duas opções ficam no Editar barra (rascunho, valem ao Concluir). O menu do Z ficou com altura,
  formato, tema, atualização, Editar barra, Configurações e Sair; o Acrílico foi para o topo da tela de
  temas; a chave do mosaico saiu (o `[wm]` continua valendo para quem edita o arquivo).
- Configurações é um painel próprio (`PanelKind::Settings`, pseudo-chip `__settings`, preso ao chip do Z)
  com itens que valem na hora: iniciar com o Windows, HUD, reiniciar a ilha, exportar diagnóstico e a
  versão no rodapé. A primeira linha, "‹ Configurações", volta ao menu do Z (o Esc também), como o
  "‹ Tema" da tela de temas.
- A bandeja: o clique esquerdo continua trazendo as barras escondidas por tela cheia; "Mostrar a barra"
  também faz a do principal descer (`AutoHide::reveal`). O menu da bandeja ficou como estava, porque com
  a barra escondida ele é o caminho que sobra.

## Consequências

- Sem a AppBar, outra AppBar no topo (de outro programa) pode ficar por baixo da barra quando ela desce.
- ~~Um monitor sem nenhum trecho de borda de verdade (outro monitor cobrindo todo o topo) só mostra a
  barra pela bandeja.~~ Substituído pela faixa de ativação (abaixo).
- A posição da faixa observada pelo hook é atualizada a cada passo da regra; mudanças de monitor relêem
  os retângulos de todos os monitores (`autohide_sync`).

## Topo coberto por outro monitor (2026-10-27)

**Problema.** A barra de um tester sumia e não voltava mais pelo mouse. O arranjo dele: o monitor 1
(principal, com `display = "principal"`) embaixo; o 2 logo acima, alinhado à esquerda e um pouco mais
estreito; o 3 em retrato à esquerda dos dois. Só a ponta direita do topo do 1, depois do fim do 2, era
borda de verdade. No resto, inclusive no meio, onde fica a barra, o cursor passava direto para o 2, e a
regra acima não deixava ele disparar. Com o 2 da mesma largura do 1, não sobraria trecho nenhum.

**Opções.** Segurar o cursor na fronteira engolindo o movimento no hook (o hook deixaria de só observar e
mudaria como o mouse anda; rejeitada); um gesto de subir e voltar (ninguém descobriria; rejeitada);
usar o topo do monitor de cima (a barra apareceria longe do cursor; rejeitada); **uma faixa de ativação no
alto do próprio monitor, nos trechos cobertos** (escolhida pelo Felipe).

**Regra.** `top_triggers` devolve os gatilhos do topo de cada monitor, em pixels de tela:

- na borda de verdade, a linha de cima, como antes (o cursor encosta e para);
- onde outro monitor está logo acima, uma faixa de **6 DIP** (`COVERED_BAND_DIP`, convertida pelo DPI do
  monitor em `covered_band`) no alto do próprio monitor. O cursor parado ali 300 ms faz a barra descer;
  subir para o monitor de cima antes disso cancela, então passar de um monitor para o outro não a traz.

Juntos, os gatilhos cobrem toda a largura do topo de qualquer monitor, em qualquer arranjo: a barra nunca
fica só pela bandeja (teste `every_point_of_the_top_reaches_the_bar`; o arranjo do tester é
`a_monitor_with_another_above_has_a_band_under_it`). O hook (`bar_context`) só confere se o cursor está
dentro de algum gatilho. 6 DIP é o meio-termo: com 4 é difícil parar ali sem passar para o monitor de cima;
com 8, parar sobre as abas de um navegador maximizado traria a barra sem querer.
