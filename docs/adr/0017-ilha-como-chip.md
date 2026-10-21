# ADR-0017: A ilha embutida vira o chip `island`

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main

## Contexto

Desde a 1c (ADR-0009), a ilha embutida ficava fixa no centro da barra do monitor principal: o layout
reservava a largura dela e o "Editar barra" travava o Centro. O usuário quer a ilha opcional e o
centro do principal livre para outros chips (no layout dele, `clock` e `weather` ficavam escondidos
atrás da ilha). Junto veio um defeito de cor: a pílula da ilha (e a do player) tinha fundo próprio a
96 % por cima da faixa, e a segunda camada deixava passar menos papel de parede. Medido nos prints:
faixa (17, 9, 10), pílulas (9, 8, 10).

## Opções consideradas

1. **Opção "ilha ligada/desligada" no menu ou no `[island]`:** libera o centro, mas a ilha continua
   presa ao principal.
2. **Chip `island` no catálogo**, em qualquer seção de qualquer monitor, como o player, o clima e o
   hardware (ADR-0012). Escolhida.

Para configs que já existem: inserir o chip sozinho desfaria o centro livre que o usuário quer; não
inserir nada faria a ilha sumir de quem não sabe da mudança. Ficou no meio: o centro padrão
(`Sections::default`) é `["island"]`, então um config antigo **sem** a chave `center` mantém a ilha;
um `center` escrito no arquivo é respeitado.

## Decisão

- `island` entra no catálogo ("Ilha"). O chip reserva a largura máxima da pílula (`RESERVE_DESIGN_W`
  na altura da barra) e a pílula fica centrada nele (`BarLayout::island_pill_rect`), então os
  vizinhos não andam quando ela espia ou mostra um aviso.
- A barra desenha a ilha onde o chip estiver (`BarView::hosts_island` olha o layout, fora do "…").
  Sem o chip, o centro do principal é uma seção como as outras.
- `Config::shows_embedded_island()` = engine embutida + chip em algum layout. Privacidade, letras,
  Bluetooth, avisos (área de transferência, Caps Lock, Não perturbe, volume, brilho) e o serviço de
  notificações seguem isso, como os outros chips seguem `BarConfig::uses`.
- A ilha externa (`engine = "externa"`) não mudou: continua no centro do principal, com a reserva e o
  Centro travado no editor; o chip `island` é ignorado com ela.
- Ilha e player usam `SurfaceKind::Merged`: sem fundo, contorno nem sombra, porque ficam sobre a
  faixa, um grupo das pílulas ou uma ilha do formato ilhas. Nas pílulas, a ilha sozinha numa seção
  fica com a cara de antes (o grupo é a pílula com contorno).

## Consequências

- Config padrão novo: `center = ["island"]`.
- Mais de um monitor pode ter a ilha; o estado (espiada, avisos) é um só e aparece em todas.
- O hover e o "ativo" não pintam fundo no chip da ilha, como no player.
