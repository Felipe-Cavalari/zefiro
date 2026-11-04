# Zefiro — símbolo refinado

Uma única silhueta funde a letra Z ao gesto de uma asa. Foram removidas as penas soltas e os elementos de brasão das propostas anteriores. A barra superior sugere a curvatura de uma asa; a diagonal mantém a leitura do Z e a base prolonga o movimento.

Em `zefiro/`:

- `color.svg`: símbolo em verde petróleo, com fundo transparente.
- `tray-dark.svg` e `tray-light.svg`: versões escura e branca.
- PNGs em 256 px e ICOs com 16, 20, 24, 32, 40, 48, 64, 128 e 256 px.

`comparativo.png` mostra o símbolo, uma composição com o nome e os tamanhos reais de bandeja. O nome é apenas uma sugestão tipográfica de apresentação, não faz parte dos SVGs do ícone.

Regenerar com `node scripts/generate-logo-concepts.cjs --refined` (requer `sharp`). Os arquivos foram verificados, mas a integração e o teste em uma bandeja Windows real ainda não foram feitos.
