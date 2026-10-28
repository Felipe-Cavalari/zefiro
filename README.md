# Zefiro

<img src="assets/branding/logo.svg" alt="Símbolo do Zefiro" width="96" height="96" />

Um ambiente de desktop nativo para Windows (barra, ilha de mídia, launcher), sem WebView: leve, fluido e personalizável.

> **Status:** em desenvolvimento (Fase 1 até a 1e: barra no design da ilha, modo de edição, ilha embutida em Rust, áudio e brilho e os widgets do YASB: uso do Claude e do Codex, projetos do VS Code, notificações, Não perturbe e energia; próxima: itens declarativos). Uso pessoal, ainda sem instalador.

- Planejamento: [`docs/PLANNING.md`](docs/PLANNING.md)
- Decisões de arquitetura: [`docs/adr/`](docs/adr/)
- Identidade visual e ícones: [`assets/branding/`](assets/branding/README.md)

## Rodar

Requisitos: Rust (a versão vem de `rust-toolchain.toml`) e o Visual Studio Build Tools com C++ (o linker que o Rust usa no Windows).

```powershell
cargo build --release -p zefiro-app
./target/release/zefiro.exe
```

- A configuração fica em `%APPDATA%\zefiro\config.toml`. Ela é criada na primeira execução e recarregada ao salvar. `[bar] height = completa | compacta` e `format = faixa | pilulas`.
- Os temas do usuário ficam em `%APPDATA%\zefiro\themes\`. **Abrir pasta de temas** (menu do Zefiro: clique em "Z Zefiro" ou botão direito na barra) copia o tema atual para lá.
- Os logs ficam em `%LOCALAPPDATA%\zefiro\logs`.

## Uso do Claude, do Codex e do Grok

Os chips de uso juntam as contas do Windows e das distros do WSL que estão rodando (`wsl --list --running`), identificando quem é quem pelo e-mail/UUID da conta em cada origem.

- O uso do Claude vem do comando `/usage` do próprio Claude Code (`claude -p /usage`), a cada 5 minutos. O Zefiro **não usa o token** da sua assinatura: quem lê o login, renova e fala com a Anthropic é o Claude Code. O comando não chama o modelo e não gasta nada da sua cota.
- Por isso o Claude Code precisa estar instalado em cada origem (Windows ou distro) com login. Sem ele, o painel diz "Claude Code não encontrado".
- Uma distro parada aparece como "WSL desligado": o Zefiro nunca lê `\\wsl.localhost\<distro>\...` de uma distro parada, porque isso a ligaria sozinho.
- O uso do Grok vem do próprio Grok Build (`grok`), a cada 5 minutos: um `grok agent stdio` curto pede a cota da assinatura (`_x.ai/billing`) sem abrir conversa e sem chamar o modelo. O chip mostra a % da cota semanal; o painel, quando ela volta e os créditos extras. É preciso ter o Grok Build instalado com `grok login` no Windows ou numa distro rodando.

## Aviso sobre a assinatura do Claude

> [!WARNING]
> O Zefiro não é um produto da Anthropic. Os termos da Anthropic dizem que o login das assinaturas Free, Pro e Max só pode ser usado no Claude Code e no Claude.ai. O Zefiro nunca lê nem usa o seu token, mas dois recursos dependem do Claude Code que você instalou:
>
> - **Chip de uso do Claude:** roda o comando `/usage` do Claude Code. Ele não chama o modelo e não gasta nada da sua cota.
> - **Chat com o Claude:** roda o `claude -p` sem ferramentas, como um chat dentro da barra, e usa a sua cota como qualquer conversa no Claude Code.
>
> Rodar o `claude -p` por script é um uso documentado do Claude Code, mas os termos não dizem nada explícito sobre outro app usar o Claude Code como chat. É uma zona cinzenta, e a Anthropic mudou essas regras várias vezes em 2026. O risco fica com a sua conta: no pior caso, a Anthropic pode bloquear o acesso ou suspender a conta. Se você não quiser correr esse risco, deixe os chips `claude` e `chat` fora da barra (modo de edição). Sem eles, o Zefiro não roda o Claude Code.

## Aviso sobre a assinatura do Grok

> [!WARNING]
> O Zefiro não é um produto da xAI. Ele nunca lê o login do Grok (`~/.grok/auth.json`) nem fala com a xAI por conta própria: os dois recursos abaixo rodam o Grok Build (`grok`) que você instalou e com o qual fez login.
>
> - **Chip de uso do Grok:** pede a cota da assinatura ao `grok` (`_x.ai/billing`). Não abre conversa, não chama o modelo e não gasta nada da sua cota.
> - **Chat com o Grok:** roda o `grok agent stdio` com um perfil que só libera a geração de imagens (sem ler arquivos, terminal ou web), como um chat dentro da barra. As imagens saem da mesma cota do Imagine. Com a assinatura (SuperGrok, X Premium+ ou X Subscription), cada pergunta sai da mesma cota semanal do grok.com, do Imagine e do Grok Build; com `XAI_API_KEY`, você paga por token. O Grok 4.7 Fast custa o dobro.
>
> Rodar o Grok Build por script ou pelo ACP é um uso documentado pela xAI, mas os termos não dizem nada explícito sobre outro app usar o Grok Build como chat com a assinatura. É uma zona cinzenta, e as regras dos planos da xAI mudaram duas vezes em 2026. O risco fica com a sua conta: no pior caso, a xAI pode limitar ou suspender o acesso. Se não quiser correr esse risco, deixe o chip `grok` fora da barra e o chat no Claude ou no Codex.

## Créditos e agradecimentos

O Zefiro nasceu de ideias, experiências de uso e trabalho compartilhado pela comunidade open source. Agradecemos aos autores, mantenedores e colaboradores de cada um destes projetos:

| Projeto | Contribuição para o Zefiro e agradecimento |
|---|---|
| [Seelen-UI](https://github.com/eythaann/Seelen-UI) | Inspiração para a proposta de um ambiente de desktop personalizável, com barra, dock, launcher, gerenciamento de janelas e recursos da comunidade. Obrigado por mostrar novas possibilidades para a experiência de desktop no Windows. A contribuição é conceitual e de experiência de uso, sem código copiado. |
| [Zebar](https://github.com/glzr-io/zebar) | Inspiração para providers reativos, pacotes de widgets e um futuro catálogo da comunidade. Obrigado por demonstrar como separar fontes de dados da apresentação e abrir espaço para a personalização. A contribuição é conceitual, sem código copiado. |
| [YASB](https://github.com/amnweb/yasb) | Referência prática para a barra e seus widgets, especialmente uso do Claude e do Codex, projetos do VS Code, brilho, Não perturbe e notificações. Obrigado pela variedade de integrações e pela experiência de personalização que ajudaram a definir as necessidades do Zefiro. |
| [PowerToys](https://github.com/microsoft/PowerToys) | Inspiração para o modo de edição da barra e para a fluidez da interação, em especial a Dock da Paleta de Comandos. Obrigado por explorar formas de tornar as ferramentas de produtividade do Windows mais acessíveis e configuráveis. A contribuição é conceitual, sem código copiado. |
| [EarTrumpet](https://github.com/File-New-Project/EarTrumpet) | Referência técnica para as interfaces internas de áudio do Windows usadas na seleção de dispositivos e no roteamento de áudio por aplicativo. Obrigado por compartilhar conhecimento sobre essas interfaces e facilitar integrações de áudio mais completas. |
| [Dynamic Island for Windows](https://github.com/devcode90/Dynamic-Island-for-Windows), de devcode90 | Inspiração para a ilha de mídia. Alguns trechos de lógica, como as molas e o visualizador, foram reescritos em Rust a partir dele e estão listados em [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). |

Nosso agradecimento também a todos que mantêm as bibliotecas Rust e as ferramentas utilizadas na construção do Zefiro. As dependências e suas versões estão registradas nos arquivos `Cargo.toml` do workspace e no [`Cargo.lock`](Cargo.lock).

Os avisos de licença e a identificação do código portado estão em [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md). Cada projeto mantém sua própria licença e autoria.

## Licença

MIT OR Apache-2.0. Veja [`LICENSE-MIT`](LICENSE-MIT) e [`LICENSE-APACHE`](LICENSE-APACHE).
