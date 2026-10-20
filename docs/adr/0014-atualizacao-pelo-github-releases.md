# ADR-0014: Atualização automática pelo GitHub Releases

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main

## Contexto

O Zefiro instalado não se atualizava: era preciso baixar o `setup.exe` novo à mão. O usuário pediu
que as releases fossem identificadas automaticamente pelo GitHub Releases, e um jeito de soltar
releases pelo GitHub. [Spec](../superpowers/specs/2026-10-20-atualizacao-automatica-design.md).

## Opções consideradas

1. **Atualizador pronto (WinSparkle, Velopack, Squirrel).** Mais recursos (deltas, rollback), mas
   dependência nativa nova e outro formato de pacote no lugar do Inno Setup.
2. **Checagem própria pela API do GitHub + o instalador Inno Setup em modo silencioso.** Usa o
   `HttpClient` do Windows que o Zefiro já usa, sem dependência nova. Escolhida.

## Decisão

- **Checagem:** thread `zefiro-update`, 60 s depois de abrir e a cada 24 h, em
  `api.github.com/repos/Felipe-Cavalari/zefiro/releases/latest` (repositório público, sem token).
  Só a cópia instalada checa (`%LOCALAPPDATA%\Programs\Zefiro`); o build de desenvolvimento, com
  `ZEFIRO_UPDATE_CHECK=1`. `[update] check = false` desliga.
- **Oferta:** tag `vX.Y.Z` maior que a versão do exe e um asset `Zefiro-*-setup.exe` → linha
  "Atualizar para a X.Y.Z" no menu do Z, na cor de destaque. Nada é baixado sem clique.
- **Segurança:** só baixa de `https://github.com/Felipe-Cavalari/zefiro/releases/download/`, confere
  o tamanho e o SHA-256 que o GitHub publica no asset (`digest`). SHA-256 próprio (FIPS 180-4,
  testado com os vetores oficiais) em `zefiro-integration-update`.
- **Instalação:** `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`; o Zefiro sai pelo caminho normal e o
  instalador o reabre (`[Run] … Check: WizardSilent`).
- **Publicação:** `.github/workflows/release.yml` num push de tag `v*`: confere a versão do
  `Cargo.toml`, roda clippy e testes, gera o instalador (Inno Setup pelo Chocolatey) e cria a
  release, ou anexa o setup à release criada no site. Guia em `docs/RELEASES.md`.

## Consequências

- Quem tem a 1.0.x precisa instalar a 1.1.0 à mão uma vez; dali em diante as versões chegam pelo
  menu do Z.
- O CI não recompila a ilha C++ (só serve a `engine = "externa"`); uma instalação antiga mantém o
  `zefiro-island.exe` que já tinha.
- A atualização silenciosa reaplica a tarefa "Iniciar com o Windows" da instalação anterior; se ela
  foi desligada só pelo menu do Z, volta a ligar.
- Sem assinatura de código, o SmartScreen pode avisar no primeiro `setup.exe` baixado pelo navegador;
  pelo atualizador, o arquivo não leva a marca da internet e o aviso não aparece.
