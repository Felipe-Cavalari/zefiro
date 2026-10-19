# Zefiro

<img src="assets/branding/logo.svg" alt="Símbolo do Zefiro" width="96" height="96" />

Um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher, dock), sem WebView: leve, fluido e personalizável.

> **Status:** em desenvolvimento (Fase 1 até a 1e: barra no design da ilha, modo de edição, ilha embutida em Rust, áudio e brilho e os widgets do YASB: uso do Claude e do Codex, projetos do VS Code, notificações, Não perturbe e energia; próxima: itens declarativos). Uso pessoal, ainda sem instalador.

- Planejamento: [`docs/PLANNING.md`](docs/PLANNING.md)
- Decisões de arquitetura: [`docs/adr/`](docs/adr/)
- Identidade visual e ícones: [`assets/branding/`](assets/branding/README.md)

## Rodar

Requisitos: Rust (a versão vem de `rust-toolchain.toml`) e o Visual Studio Build Tools com C++ (para a ilha).

```powershell
cargo build --release -p zefiro-app
./island/build.ps1
Copy-Item island/build/zefiro-island.exe target/release/
./target/release/zefiro.exe
```

- A configuração fica em `%APPDATA%\zefiro\config.toml`. Ela é criada na primeira execução e recarregada ao salvar. `[bar] height = completa | compacta` e `format = faixa | pilulas`.
- Os temas do usuário ficam em `%APPDATA%\zefiro\themes\`. **Abrir pasta de temas** (menu do Zefiro: clique em "Z Zefiro" ou botão direito na barra) copia o tema atual para lá.
- Os logs ficam em `%LOCALAPPDATA%\zefiro\logs`.
- Sem o `zefiro-island.exe` ao lado do `zefiro.exe`, a barra abre normalmente, com o centro vazio.

Licença: MIT OR Apache-2.0.
