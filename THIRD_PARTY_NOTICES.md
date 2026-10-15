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
| [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows) | MIT | `Copyright (c) 2026 devcode90` | [`island/`](island/) (licença original em [`island/LICENSE`](island/LICENSE)) | O mod inteiro (`dynamic-island-for-windows.wh.cpp`, v1.3.1, commit upstream `e6cfedb`), compilado como `zefiro-island.exe`. Alterações do Zefiro marcadas com `// ZEFIRO:`. Regras de tokens de material e de contraste do destaque, portadas para Rust em `crates/resources/src/tokens.rs`. Constantes e integrador das molas em `crates/ui/src/spring.rs`. Layout do calendário em `crates/modules/bar/src/calendar.rs`. |
