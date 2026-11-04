# Contribuindo com o Zefiro

Obrigado pelo interesse! O Zefiro é um projeto pessoal em beta, então o jeito mais útil de ajudar hoje é **usar e reportar problemas** (com o diagnóstico exportado, veja o [README](README.md#reportar-um-problema)).

## Antes de escrever código

1. Abra uma issue (ou comente numa existente) descrevendo a ideia em poucas frases e espere um "sim". Mudanças grandes (chip novo, formato, integração) começam por uma spec em `docs/superpowers/specs/`, como as das fases anteriores.
2. Leia o [`AGENTS.md`](AGENTS.md): ele tem a estrutura do repositório, as regras e as "coisas não óbvias" já descobertas. Leia também os ADRs em [`docs/adr/`](docs/adr/) antes de mexer em desenho, animação, janelas ou na ilha. Para desfazer uma decisão de um ADR, escreva outro ADR que a substitua.

## Regras que valem para todo PR

- **Idioma:** documentos, comentários, mensagens de log e textos da interface em **português do Brasil**; identificadores, branches e commits em **inglês**.
- **Commits** no estilo `feat(bar): …`, `fix(platform-windows): …`, `docs(adr): …`.
- **A crate `windows` só pode aparecer** em `crates/platform-windows`, em `crates/integrations/clock/src/windows.rs` e em `crates/app`. As demais crates são portáveis e compilam no Linux. O script confere:

  ```powershell
  pwsh -NoProfile -File scripts/check-windows-imports.ps1
  ```

- **Lint e testes** precisam passar:

  ```powershell
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

- **`unsafe`** só em blocos pequenos, cada um com um comentário `// SAFETY:` explicando por que está certo.
- **Testes primeiro:** escreva o teste, veja falhar, implemente. Para comportamento na tela, uma sonda que falha antes e passa depois também vale.
- **Animações** rodam no compositor (molas e expressões), nunca redesenhando a cada tick de um temporizador.
- **Privacidade:** nenhum token, conversa ou texto digitado vai para o log, e nada sai do PC sem a pessoa pedir.

## Testando sem mexer na sua configuração

Aponte `ZEFIRO_CONFIG_DIR` para uma pasta vazia:

```powershell
$env:ZEFIRO_CONFIG_DIR = "$env:TEMP\zefiro-teste"
cargo run --release -p zefiro-app
```

Feche o YASB (ou outra barra) antes, e feche o Zefiro pelo **Sair** do menu do Z para devolver a área da AppBar ao Windows.

## Licença

Ao contribuir, você concorda que o seu código seja licenciado como o restante do projeto: **MIT OR Apache-2.0**. Se portar código de outro projeto, confira a licença dele e registre em [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md); código AGPL ou GPL não pode entrar.
