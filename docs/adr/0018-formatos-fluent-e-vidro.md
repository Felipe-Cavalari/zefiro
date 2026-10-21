# ADR-0018: Formatos Fluent e Vidro

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main

## Contexto

Depois das ilhas (ADR-0016), o usuário pediu mais dois formatos, tirados de temas do YASB: "Win11
Fluent Onyx" (Hoxiee) e "Frosted Glass Revamp" (Harishkumar-ghub). O CSS dos dois está no
repositório `amnweb/yasb-themes`. O mockup aprovado (artifact "Formatos Fluent e Vidro") fixou as
medidas abaixo.

- **Win11 Fluent Onyx:** `.yasb-bar` de ponta a ponta com `border-bottom` de 1 px; cada widget num
  `.widget-container` com `border-radius` 12, borda de 1 px e fundo `rgba(44, 44, 44, 0.4)`.
- **Frosted Glass Revamp:** `.yasb-bar` com `border-radius` 16, borda `rgba(255, 255, 255, 0.12)`
  e fundo translúcido; `.widget` com `border-radius` 10 e fundo `rgba(255, 255, 255, 0.03)`.

## Opções consideradas

1. **Temas novos** (cores e material): não dão a geometria; a barra flutuante e os cartões não são
   cor.
2. **Formatos** (`[bar] format`), com as cores vindo do tema. Escolhida: o Vidro funciona com
   qualquer tema, e o acrílico (agora no menu do Z) dá o desfoque.

## Decisão

- `format = "fluent"`: a faixa de 36/32 com a linha embaixo; cada chip num cartão de 28/24 com canto
  8, o fundo realçado do tema (`raised`) e borda `hairline`; margem 12 dentro do cartão; o mouse em
  cima usa `raised_strong`. Sem cartão: Z, relógio e energia (como no tema) e a ilha e o player
  (já são pílulas).
- `format = "vidro"`: a barra flutua a 6 DIP do topo e das laterais, com 34/30 de altura, canto 16 e
  o estilo de pílula do tema (borda e sombra). A AppBar reserva 46/42 (a barra e 6 de cada lado),
  então as janelas maximizadas ficam a 6 dela. Os chips têm canto 10 e um fundo de 35 % do `raised`
  (o 3 % de branco do tema no Obsidian); o mouse em cima usa o `raised`.
- O layout portátil diz o resto: `BarLayout::content_top/content_h` (a faixa útil, que também serve
  ao trilho das ilhas), `strip_radius`, `divider`, `chip_backing` (`None`, `Card`, `Frost`),
  `chip_radius` e `has_backing(id)`. A barra usa o retângulo de hover de cada chip como o fundo de
  sempre.
- A linha Formato do menu do Z tem cinco opções. A ilha externa segue a faixa no Fluent (44/28) e as
  pílulas no Vidro (36/24), sempre `merged`.

## Consequências

- Da faixa para o Vidro, a mesma forma da faixa encolhe das bordas com a mola até virar a barra
  flutuante; para o Fluent, os cartões aparecem com a mola de opacidade.
- Workspaces, apps abertos, rede e discos (widgets do YASB que aparecem nos prints) continuam fora.
- Temas com as cores dos prints ficam para depois, sem mexer nos formatos.
