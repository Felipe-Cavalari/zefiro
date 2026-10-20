# ADR-0012: Chips da ilha (player, clima e monitor do PC)

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main (commits 1147c4f … 8c6d62d)

## Contexto

O usuário queria a ilha também no monitor secundário, mas opcional, e não fixa no centro como no
principal. A [spec](../superpowers/specs/2026-10-20-chips-da-ilha-design.md) registra a conversa.

## Opções consideradas

1. **Uma segunda ilha completa no secundário** (um item `island` no catálogo). Com o layout
   compartilhado, o item cairia também no principal, onde a ilha já é fixa, e o app inteiro supõe uma
   ilha só (`island_bar`, avisos, captura). Descartada pelo usuário.
2. **Três chips comuns com partes da ilha**: `player`, `weather` e `hardware`, em qualquer seção de
   qualquer monitor. Escolhida.

## Decisão

- **`player`** desenha a mesma pílula de mídia da ilha numa `IslandView` própria por barra, no
  retângulo do chip: mesma mola de morph, mesmo visualizador, a mesma espiada na troca de faixa. Ele
  **não** mostra os avisos da ilha (volume, notificações, timer, privacidade). O hover espia só aquele
  chip (`IslandState::player`), sem fazer a ilha espiar.
- **A largura do player segue a pílula** (parado < mídia < espiando). Quando ela muda, a barra refaz
  o layout e os vizinhos deslizam com a mola (`BarView::refresh_media`). O player é um chip "largo"
  (`ChipSpec::wide`), sem o teto de 160 DIP. Uma reserva fixa na largura da espiada deixaria um
  buraco de ~110 px com a mídia recolhida.
- **Painéis com abas:** `PanelKind::{Player, Weather, Hardware}` reaproveitam o painel da ilha, cada
  um com uma aba (`tabs::for_set`). O chip sem o que mostrar (player parado, clima sem leitura) conta
  como clique fora.
- **Serviços:** mídia e clima rodam com a ilha embutida **ou** com o chip em algum layout; a captura
  de áudio roda com mídia tocando e uma ilha ou um player visível. `refresh_services()` agora também
  chama `island_services()`.
- **Clima com largura estável:** a temperatura reserva `-00°` (regra do ADR-0010); a cidade é
  cortada com "…" para caber em 160 DIP.

## Consequências

- A ilha do principal não muda e continua fixa no centro.
- O player não tem fundo de hover nem de ativo (é uma pílula, como a ilha), e o botão direito nele
  abre o menu do Zefiro, não o menu da ilha.
- Espiar refaz o layout da barra (redesenha os chips dela). Acontece só na troca de faixa e no hover
  do player, e não por quadro.
- Com o painel do player aberto, o hover não espia, para o painel não andar sob o cursor; ao fechar,
  o player volta ao tamanho recolhido.
