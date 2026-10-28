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

- O uso do Claude vem do comando `/usage` do próprio Claude Code (`claude -p /usage`), a cada 5 minutos. O Zefiro **não usa o token** da sua assinatura: quem lê o login, renova e fala com a Anthropic é o Claude Code. O comando não chama o modelo e não gasta nada da sua cota.
- Por isso o Claude Code precisa estar instalado em cada origem (Windows ou distro) com login. Sem ele, o painel diz "Claude Code não encontrado".
- Uma distro parada aparece como "WSL desligado": o Zefiro nunca lê `\\wsl.localhost\<distro>\...` de uma distro parada, porque isso a ligaria sozinho.

## Aviso sobre a assinatura do Claude

> [!WARNING]
> O Zefiro não é um produto da Anthropic. Os termos da Anthropic dizem que o login das assinaturas Free, Pro e Max só pode ser usado no Claude Code e no Claude.ai. O Zefiro nunca lê nem usa o seu token, mas dois recursos dependem do Claude Code que você instalou:
>
> - **Chip de uso do Claude:** roda o comando `/usage` do Claude Code. Ele não chama o modelo e não gasta nada da sua cota.
> - **Chat com o Claude:** roda o `claude -p` sem ferramentas, como um chat dentro da barra, e usa a sua cota como qualquer conversa no Claude Code.
>
> Rodar o `claude -p` por script é um uso documentado do Claude Code, mas os termos não dizem nada explícito sobre outro app usar o Claude Code como chat. É uma zona cinzenta, e a Anthropic mudou essas regras várias vezes em 2026. O risco fica com a sua conta: no pior caso, a Anthropic pode bloquear o acesso ou suspender a conta. Se você não quiser correr esse risco, deixe os chips `claude` e `chat` fora da barra (modo de edição). Sem eles, o Zefiro não roda o Claude Code.

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
