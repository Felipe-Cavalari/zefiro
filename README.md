# Zefiro

<img src="assets/branding/logo.svg" alt="Símbolo do Zefiro" width="96" height="96" />

Um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher), sem WebView: leve, fluido e personalizável.

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

## Uso do Claude e do Codex

Os chips de uso juntam as contas do Windows e das distros do WSL que estão rodando (`wsl --list --running`), identificando quem é quem pelo e-mail/UUID da conta em cada origem.

- O Zefiro **só lê** o arquivo de credenciais (`~/.claude/.credentials.json`) a cada consulta; ele nunca renova nem grava o token, porque isso derrubaria o login do Claude Code. Quem renova é sempre o próprio Claude Code, ao ser executado.
- Isso significa que o painel pode mostrar **"Login expirado"** numa conta do WSL mesmo com a distro ligada: o token OAuth tem validade própria e só é renovado quando o `claude` roda de fato lá dentro — ter a distro "rodando" em segundo plano, sem nenhum processo usando o Claude Code, não renova nada sozinho.
- Para manter a conta do WSL sempre válida sem depender de um terminal aberto, deixe algo rodando o Claude Code na distro (um serviço systemd, uma sessão `tmux`/`screen`), em vez de contar com o Windows Terminal ficar aberto.
- Uma distro parada aparece como "WSL desligado": o Zefiro nunca lê `\\wsl.localhost\<distro>\...` de uma distro parada, porque isso a ligaria sozinho.

## Créditos e agradecimentos

O Zefiro nasceu de ideias, experiências de uso e trabalho compartilhado pela comunidade open source. Agradecemos aos autores, mantenedores e colaboradores de cada um destes projetos:

| Projeto | Contribuição para o Zefiro e agradecimento |
|---|---|
| [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows), de devcode90 e colaboradores | Base da ilha de mídia: o fork em C++ está em [`island/`](island/), e a implementação em Rust incorpora lógica de molas, visualizador, cor da capa e regras visuais do projeto. Obrigado por compartilhar uma ilha tão completa e por tornar possível construir sobre esse trabalho. |
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | Inspiração para a proposta de um ambiente de desktop personalizável, com barra, dock, launcher, gerenciamento de janelas e recursos da comunidade. Obrigado por mostrar novas possibilidades para a experiência de desktop no Windows. A contribuição é conceitual e de experiência de uso, sem código copiado. |
| [Zebar](https://github.com/glzr-io/zebar) | Inspiração para providers reativos, pacotes de widgets e um futuro catálogo da comunidade. Obrigado por demonstrar como separar fontes de dados da apresentação e abrir espaço para a personalização. A contribuição é conceitual, sem código copiado. |
| [YASB](https://github.com/amnweb/yasb) | Referência prática para a barra e seus widgets, especialmente uso do Claude e do Codex, projetos do VS Code, brilho, Não perturbe e notificações. Obrigado pela variedade de integrações e pela experiência de personalização que ajudaram a definir as necessidades do Zefiro. |
| [PowerToys](https://github.com/microsoft/PowerToys) | Inspiração para o modo de edição da barra e para a fluidez da interação, em especial a Dock da Paleta de Comandos. Obrigado por explorar formas de tornar as ferramentas de produtividade do Windows mais acessíveis e configuráveis. A contribuição é conceitual, sem código copiado. |
| [EarTrumpet](https://github.com/File-New-Project/EarTrumpet) | Referência técnica para as interfaces internas de áudio do Windows usadas na seleção de dispositivos e no roteamento de áudio por aplicativo. Obrigado por compartilhar conhecimento sobre essas interfaces e facilitar integrações de áudio mais completas. |
| [Windhawk](https://github.com/ramensoftware/windhawk) e [Windhawk Mods](https://github.com/ramensoftware/windhawk-mods) | Ecossistema em que o mod Dynamic Island for Windows se desenvolveu, antes de sua adaptação para um executável independente no Zefiro. Obrigado por oferecer uma plataforma e uma comunidade para experimentar e compartilhar melhorias no Windows. |

Nosso agradecimento também a todos que mantêm as bibliotecas Rust e as ferramentas utilizadas na construção do Zefiro. As dependências e suas versões estão registradas nos arquivos `Cargo.toml` do workspace e no [`Cargo.lock`](Cargo.lock).

Os avisos de licença e a identificação do código portado estão em [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md); a licença original da ilha está em [`island/LICENSE`](island/LICENSE). Cada projeto mantém sua própria licença e autoria.

## Licença

MIT OR Apache-2.0. Veja [`LICENSE-MIT`](LICENSE-MIT) e [`LICENSE-APACHE`](LICENSE-APACHE).
