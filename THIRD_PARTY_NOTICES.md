# Avisos de terceiros

## Inspiração (nenhum código copiado)

| Projeto | Licença | O que inspirou |
|---|---|---|
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | AGPL-3.0 | Módulos (toolbar, dock, WM, launcher), recursos `@autor/nome`, app de configurações. **Só ideias.** |
| [Zebar](https://github.com/glzr-io/zebar) | GPL-3.0 | Modelo de providers reativos, pacotes de widgets. **Só ideias.** |
| PowerToys (Dock da Paleta de Comandos) | MIT | Modo de edição e fluidez. **Só ideias.** |

## Código portado

| Projeto | Licença | Copyright | Onde está | O que foi portado |
|---|---|---|---|---|
| [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows) | MIT | `Copyright (c) 2026 devcode90` | Trechos reescritos em Rust (o fork em C++ saiu do repositório na ADR-0026) | Regras de tokens de material e de contraste do destaque em `crates/resources/src/tokens.rs`. Constantes e integrador das molas em `crates/ui/src/spring.rs`. Layout do calendário em `crates/modules/bar/src/calendar.rs`. Na ilha (`crates/modules/island`): os tempos dos estados, o visualizador de barras, o analisador de espectro e a cor da capa. |

### Licença do Dynamic Island for Windows

```
MIT License

Copyright (c) 2026 devcode90

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
