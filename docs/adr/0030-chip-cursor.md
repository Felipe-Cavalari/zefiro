# ADR-0030: Chip `cursor`, os projetos recentes do Cursor

- **Status:** aceito (complementa o ADR-0011)
- **Data:** 2026-11-03
- **Spike/branch:** main

## Contexto

O chip `vscode` (ADR-0011) lista os projetos recentes do VS Code e abre o escolhido. O usuário usa
também o Cursor e pediu o mesmo chip para ele, com o ícone exato do Cursor.

O Cursor é um fork do VS Code. Conferido na máquina do usuário: `%APPDATA%\Cursor\User\workspaceStorage`
tem a mesma estrutura (`<hash>\workspace.json` com `folder` ou `workspace`, e o `state.vscdb` ao lado),
as pastas do WSL aparecem como `vscode-remote://wsl+<distro>/...` e o `Cursor.exe` (em
`%LOCALAPPDATA%\Programs\cursor`, com `bin\cursor.cmd` ao lado) aceita `--folder-uri` e `--file-uri`.

## Opções consideradas

1. **Copiar a crate e o painel do VS Code para o Cursor.** Duplicaria a leitura, o painel (busca,
   rolagem, hover, teclado) e a ligação no laço de eventos, que seriam iguais linha a linha.
2. **Um `Editor` (VS Code ou Cursor) que parametriza o que muda de verdade.** Escolhida.
3. **Ícone: SVG monocromático tingido, como o do VS Code.** O logo do Cursor é um cubo cinza num
   quadrado preto: tingido com a cor do tema perde o desenho. Desenhar uma imitação ou baixar o
   arquivo de uma fonte de terceiros também está fora de questão.

## Decisão

- `zefiro_integration_vscode::Editor { Code, Cursor }` guarda o que muda: o nome na interface, a
  pasta de dados (`Code` ou `Cursor`, em `%APPDATA%`), o `.cmd` do PATH, o executável e as pastas
  padrão de instalação. `read_recents`, `describe` e `open_args` são os mesmos. A crate mantém o nome.
- `PanelKind::Cursor` e o id `cursor` no catálogo ("Cursor", "Projetos recentes do Cursor"), que
  entram em `[bar.sections]` e no modo de edição como os outros chips.
- O painel (`zefiro_bar::vscode::vscode_panel`) recebe o nome do editor para o título ("Cursor ·
  recentes"). O `vscode_loop.rs` atende os dois painéis com um estado só (só um painel abre por vez):
  o `Editor` do painel aberto escolhe a pasta de leitura e o executável; uma leitura antiga do outro
  editor que chegue tarde é ignorada. Sem o Cursor instalado, o painel diz "Cursor não encontrado".
- `system::find_editor_exe(Editor)` substitui o `find_code_exe`: procura o `.cmd` no PATH e depois as
  pastas padrão (`%LOCALAPPDATA%\Programs\cursor\Cursor.exe`, `%ProgramFiles%\Cursor\Cursor.exe`).
- O ícone do chip é o do próprio `Cursor.exe` do usuário, lido por `app_icon::file_icon` (o
  `PrivateExtractIconsW` pega o tamanho mais próximo de 64 px do `.ico` do executável, em vez de
  ampliar o de 32). Uma thread o lê uma vez, com o chip na barra, e o chip o desenha como imagem
  colorida (`with_image_icon`, como o Terminal faz com os ícones dos perfis); até chegar, ou sem
  Cursor, o chip mostra o glifo `\u{E8B7}`. Nenhum arquivo de marca entra no repositório.
- Sonda: `cargo run -p zefiro-platform-windows --example vscode_probe -- cursor`.

## Consequências

- Painel fechado = zero custo; o único trabalho do chip é uma leitura de ícone, uma vez por execução.
- Um Cursor instalado depois que a barra abriu ganha o ícone na próxima vez que a configuração for
  salva (o glifo fica até lá); o painel o acha na hora, porque procura o executável ao abrir.
- A cor do ícone não segue o tema (é a marca do app). Num tema muito claro o quadrado preto do
  Cursor aparece como é.
- Outro fork do VS Code (Windsurf, VSCodium) entra com mais uma variante de `Editor` e mais um id.
