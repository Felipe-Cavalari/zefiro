# Zefiro — propostas de logo

Quatro direções para avaliação; nenhuma está definida como identidade oficial.

1. **Z corrente:** inicial Z com cortes e direção de vento. A opção mais direta e legível em tamanhos pequenos.
2. **Asa do oeste:** três penas em movimento. Evoca a representação alada do vento e a leveza do projeto.
3. **Sopro contínuo:** correntes de ar com curvas abertas. Comunica fluidez e movimento de forma imediata.
4. **Elmo alado:** símbolo mitológico estilizado, com asa e perfil de elmo. Uma interpretação livre de Zéfiro, sem pretensão de reconstrução histórica.

Veja `comparativo.png` para comparar as propostas, inclusive em 16, 20, 24 e 32 pixels.

## Arquivos de cada proposta

- `color.svg`: matriz vetorial colorida, sem fundo.
- `tray-dark.svg`: símbolo escuro para superfícies claras.
- `tray-light.svg`: símbolo branco para superfícies escuras.
- `.png`: prévias transparentes em 256 × 256.
- `.ico`: contêineres com imagens em 16, 20, 24, 32, 40, 48, 64, 128 e 256 pixels.

Os SVGs usam apenas geometria vetorial, sem fontes, filtros, imagens vinculadas ou dependências externas. O quadro colorido do comparativo é apenas uma apresentação; os ícones têm fundo transparente.

No Windows, use o ICO no recurso do executável e converta/carregue o ICO adequado como HICON para a bandeja. SVG é a matriz de edição, não um substituto direto de HICON. A integração e a escolha automática entre variantes claras/escuras ainda não foram implementadas. Após escolher uma proposta, os tamanhos menores podem receber ajustes ópticos específicos.

## Regeneração

Execute `node scripts/generate-logo-concepts.cjs` com o pacote `sharp` disponível no ambiente (ou via `NODE_PATH`). O script gera os SVGs, PNGs, ICOs e o comparativo sem alterar os recursos do aplicativo.
