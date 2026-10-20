# Atualização automática pelo GitHub Releases — design

Data: 2026-10-20. Pedido do usuário: "identifique automático os releases, utilizando o GitHub Releases"
e, depois, um guia de como soltar releases pelo GitHub.

## Como funciona

1. **Checagem:** uma thread (`zefiro-update`) consulta
   `https://api.github.com/repos/Felipe-Cavalari/zefiro/releases/latest` 60 s depois de abrir e a cada
   24 h. O repositório é público: sem token. Drafts e pré-releases não contam (a API de "latest" já
   ignora).
2. **Versão nova:** a tag (`v1.2.3`) é comparada com a versão do exe (`CARGO_PKG_VERSION`). Se for
   maior e a release tiver um asset `Zefiro-<versão>-setup.exe`, aparece **"Atualizar para a 1.2.3"**
   no topo das ações do menu do Z, na cor de destaque.
3. **Instalar:** o clique baixa o setup (só de
   `https://github.com/Felipe-Cavalari/zefiro/releases/download/…`), confere o tamanho e o SHA-256
   que o GitHub publica no asset (`digest`), grava em `%TEMP%\zefiro-update\` e roda
   `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`. O Zefiro sai pelo caminho normal (AppBar liberada); o
   instalador atualiza e abre o Zefiro de novo.
4. **Nada é baixado sem clique.** Falha de rede ou de verificação vira "Atualização falhou; tentar de
   novo" no menu e um aviso no log.

## Decisões

- Só a cópia instalada (`%LOCALAPPDATA%\Programs\Zefiro`) checa; o build de desenvolvimento
  (`target\release`) não, a não ser com `ZEFIRO_UPDATE_CHECK=1`.
- `[update] check = true` no `config.toml` (padrão ligado) desliga a checagem.
- O instalador ganha uma entrada `[Run]` que reabre o Zefiro só na instalação silenciosa.
- Lógica portável (versão, leitura do JSON, escolha do asset, URL confiável, SHA-256) numa crate
  nova `zefiro-integration-update`, testada com o JSON real da v1.0.1.
- **Releases pelo GitHub:** um workflow `release.yml` que, num push de tag `v*`, confere a versão do
  `Cargo.toml`, gera o instalador no `windows-latest` (Inno Setup pelo Chocolatey) e cria a release
  com o setup anexado. O guia fica em `docs/RELEASES.md`.

## Não objetivos

- Atualização em segundo plano sem clique, canais beta, rollback.
- Recompilar a ilha C++ no CI (só serve a `engine = "externa"`).
