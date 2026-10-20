# ADR-0016: Formato ilhas (Adaptive Islands do YASB)

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main

## Contexto

O usuário quer formatos novos de barra, começando pelo "Adaptive Islands" do tema dele no YASB: um
trilho preto fino no topo, de onde descem três ilhas (início, centro e fim) com os cantos de baixo
arredondados e concordâncias côncavas onde encontram o trilho. Entre as ilhas aparece o papel de
parede. Valores do tema: `railheight 4`, `islandradius 16`, `edgeradius 8`, `grouppadding 12`. O
mockup aprovado (artifact "Formato Ilhas do Zefiro") fixou as decisões abaixo; o usuário pediu para
implementar direto, sem spec e plano separados.

## Opções consideradas

Para desenhar a silhueta (trilho + ilhas + concordâncias) no compositor:

1. **Um caminho só** (`CompositionPathGeometry`) com a silhueta inteira: fiel, mas o caminho não anima
   com mola (só `PathKeyFrameAnimation`), e qualquer mudança de largura seria um salto.
2. **Trilho de ponta a ponta + ilhas por cima:** simples, mas as peças se sobrepõem nos 4 px do topo.
   Com a opacidade do tema (0,96) ou o acrílico (preenchimento 0,45), a sobreposição fica mais escura.
3. **Peças que não se sobrepõem:** cada ilha é um `MorphShape` (retângulo arredondado com mola) que
   começa um raio acima da tela, para os cantos de cima sumirem; o trilho são retângulos só nos vãos;
   as concordâncias são formas pequenas fixas que seguem a lateral da ilha por expressão.

Para a concordância com acrílico: `CompositionMaskBrush` não aceita o fundo do Windows como fonte
(`E_INVALIDARG`, "Unsupported source brush type"). Um caminho do Direct2D entregue ao compositor
(`CompositionPath` via `IGeometrySource2DInterop`) serve ao mesmo tempo de forma e de recorte do fundo.

## Decisão

Opção 3, com as concordâncias como `CompositionPath` (`platform-windows/src/fillet.rs`). O layout
portátil (`zefiro_ui::bar_layout`) produz as ilhas (`groups`), os pedaços do trilho e as
concordâncias (`Islands`), e `BarLayout::group_shape` estende a ilha para cima e para fora da borda do
monitor (o canto externo das ilhas das pontas fica reto, como no YASB).

- `[bar] format = "ilhas"` e a terceira opção da linha Formato no menu do Z.
- Medidas em DIP: altura 36/32 (completa/compacta, reservada inteira na AppBar), trilho 4, canto 16,
  concordância 8, margem interna 12, chips 28/24 centrados abaixo do trilho.
- Seção vazia não tem ilha; o trilho passa direto. Um vão menor que duas concordâncias fica sem elas.
- No monitor principal, a ilha do centro envolve a reserva da ilha de mídia mais 12 de cada lado; a
  pílula da mídia vai sem contorno e sem sombra e fica centrada abaixo do trilho
  (`BarLayout::island_pill_y`).
- Trilho, ilhas, concordâncias, ilha de mídia e player usam `SurfaceKind::Island`: a cor e o fundo do
  tema, sem profundidade (o degradê de cada peça faria emenda com a vizinha), sem contorno e sem sombra.
- Os grupos ficam por seção (`BarLayout::group_slot`): uma ilha ou pílula nasce e some no próprio
  lugar. Isso também vale para as pílulas, que antes eram casadas pela ordem.
- Da faixa para as ilhas, ilhas e trilho nascem do retângulo da faixa e a faixa some na hora; o trilho
  encolhe até 4 px e as ilhas arredondam embaixo com a mola. Das pílulas para as ilhas, as pílulas
  sobem e grudam no trilho.
- Ilha externa (`engine = "externa"`): pílula 36/24 e `merged` (sem contorno), como na faixa.
- Para nascer num lugar, a forma usa `MorphShape::animate_from` (a mola recebe `InitialValue`). O
  `set` + `animate` de antes não funcionava: a parada do `set` chega ao compositor depois e a mola sai
  do valor antigo. Visto na tela: da faixa para as ilhas, o trilho aparecia já fino, sem encolher. As
  pílulas vindas da faixa usam o mesmo caminho e também passam a nascer do retângulo da faixa.

## Consequências

- Clique: as ilhas e o trilho recebem clique; os vãos passam para a área de trabalho.
- A ilha externa não desce abaixo do trilho (a posição é do C++); fica 2 px mais alta que a embutida.
- Os valores são fixos. Deixar trilho, canto e margem configuráveis é da Fase 5 (declarativo).
- Os widgets do YASB que aparecem no print (busca, desktops, barra de tarefas) ficaram de fora: as ilhas
  mostram os chips do catálogo.
- Durante a mola, as peças andam com a mesma mola a partir de posições que se encostam, então
  continuam encostadas; uma mola interrompida no meio pode deixar uma fresta de subpixel até assentar.
- Outros formatos novos entram como mais uma variante de `BarFormat`, com a sua linha em `metrics`.
