# Identidade visual do Zefiro

Símbolo oficial aprovado em 15 de outubro de 2026: um Z de curvas contínuas que sugere uma asa e o movimento do vento.

## Matriz e cor

`logo.svg` é a fonte oficial da geometria. Fundo transparente, área de desenho de 64 × 64 e cor principal **verde petróleo `#087F85`**. Preserve a proporção e o espaço interno do SVG.

## Recursos para o aplicativo

Os arquivos de uso do aplicativo ficam em `../icons/`:

| Nome | Uso |
| --- | --- |
| `app.svg`, `app.png`, `app.ico` | Símbolo colorido para o aplicativo e recurso do executável |
| `tray-dark.svg`, `tray-dark.png`, `tray-dark.ico` | Símbolo escuro sobre uma bandeja clara |
| `tray-light.svg`, `tray-light.png`, `tray-light.ico` | Símbolo branco sobre uma bandeja escura |

Os PNGs têm 256 × 256 pixels. Cada ICO contém 16, 20, 24, 32, 40, 48, 64, 128 e 256 pixels. Use ICO como base do recurso do executável e de HICON da bandeja. A variante da bandeja deve acompanhar a cor da superfície onde aparece.

Para regenerar: `node scripts/generate-icons.cjs` a partir da raiz do projeto, com `sharp` disponível no ambiente. Edite a matriz, depois regenere os derivados.

Estes são os recursos oficiais usados pelo executável, pelo instalador e pelo ícone da bandeja.

As pastas `concepts/`, `z-alado/` e `zefiro-refinado/` preservam a exploração visual. Não são a fonte dos recursos oficiais. A composição com o nome na prévia refinada é uma sugestão tipográfica de apresentação, não um logotipo tipográfico definido.
