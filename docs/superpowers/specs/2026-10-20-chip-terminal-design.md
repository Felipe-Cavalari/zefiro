# Chip Terminal: perfis do Windows Terminal — design

Data: 2026-10-20. Aprovado em conversa ("Spec aprovada"); desenho B escolhido com os mockups.

## Intenção

O usuário abre o Windows Terminal pela lista de atalhos da barra de tarefas ("Tarefas": PowerShell,
Windows PowerShell, Prompt de comando, Ubuntu-24.04) e quer o mesmo atalho na barra do Zefiro: um
chip com ícone de terminal que, ao clicar, mostra os perfis e abre o escolhido. Além disso, quer
**escolher quais perfis aparecem e em que ordem**, pela interface e não editando arquivo, **no próprio
painel** (lápis → edição → Concluir), não no "Editar barra".

## O chip

| id | Nome | Na barra | Clique |
| --- | --- | --- | --- |
| `terminal` | Terminal | Só o ícone (Segoe Fluent "CommandPrompt", `E756`), largura fixa como os chips de som e VS Code. | Abre o painel "Terminal". |

Descrição no catálogo: "Perfis do Windows Terminal; o lápis escolhe e ordena". Entra em qualquer
seção de qualquer monitor pelo modo de edição, como os outros chips.

## O painel, modo normal

- Nasce do chip, como o do VS Code. Título "Terminal", subtítulo "Perfis do Windows Terminal" e, à
  direita do título, um botão de lápis (`E70F`).
- Uma linha por perfil **visível**, na ordem escolhida: ícone do perfil (18 DIP) e nome. O perfil
  padrão do Terminal (`defaultProfile`) leva um selo apagado "padrão".
- Clique numa linha (ou setas + Enter) abre o perfil e fecha o painel.
- Sem busca nem rolagem: a lista é curta. *(Chamada minha: se passar de 12 perfis visíveis, rola com a
  roda como o do VS Code.)*
- Mensagens no lugar da lista: "Carregando…"; "Windows Terminal não encontrado"; "Não consegui ler as
  configurações do Windows Terminal"; "Nenhum perfil visível. Toque no lápis para escolher."

## O painel, modo de edição

- O lápis vira o botão "Concluir"; o subtítulo vira "Arraste para ordenar · o olho esconde".
- Lista de cima, os visíveis: cada linha ganha uma alça (`⋮⋮`) à esquerda e um olho à direita.
- Rótulo "Ocultos" e, abaixo, os perfis escondidos, apagados (~40%), com o olho fechado, na ordem do
  Windows Terminal.
- **Arrastar** uma linha pela alça (ou pela linha toda) reordena os visíveis; soltar na área "Ocultos"
  esconde; arrastar um oculto para cima mostra de novo, na posição em que foi solto. Durante o
  arraste: a linha fica fantasma no lugar, uma cópia segue o mouse e um traço com a cor de destaque
  marca onde ela vai cair — o mesmo vocabulário do "Editar barra".
- **Clicar no olho** esconde (vai para o fim de "Ocultos") ou mostra (vai para o fim dos visíveis).
- Clique na linha, fora do olho, **não abre** o perfil no modo de edição.
- Teclado *(chamada minha)*: setas escolhem a linha; Ctrl+↑/↓ move a escolhida; Espaço esconde ou
  mostra; Enter conclui; Esc cancela.
- **Concluir** grava e volta ao modo normal. **Esc** descarta as mudanças e volta ao modo normal.
  **Fechar o painel** no meio da edição (clique fora, chip de novo) *grava*, para nada se perder
  *(chamada minha)*.
- O painel muda de altura ao entrar e sair da edição (a seção "Ocultos" aparece); a mudança usa a
  mola do painel e o painel continua ancorado no chip.
- Pode esconder todos: o modo normal mostra a mensagem do lápis.

## De onde vêm os perfis

Lidos **numa thread ao abrir o painel** (como o VS Code); nada vigia arquivo nem roda com o painel
fechado.

1. **`settings.json` do Windows Terminal**, o primeiro que existir:
   - `%LOCALAPPDATA%\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json`;
   - `%LOCALAPPDATA%\Packages\Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe\LocalState\settings.json`;
   - `%LOCALAPPDATA%\Microsoft\Windows Terminal\settings.json` (instalação sem pacote).
2. O arquivo é JSON com comentários e vírgulas sobrando: tirar `//`, `/* */` e vírgulas finais antes
   do `serde_json`, sem mexer no que estiver dentro de strings.
3. `profiles` pode ser uma lista ou um objeto `{ "defaults": …, "list": [ … ] }`.
4. **Fragmentos** em `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\*\*.json` e
   `%PROGRAMDATA%\Microsoft\Windows Terminal\Fragments\*\*.json`: completam os perfis do
   `settings.json` com o mesmo `guid` (é de lá que vem o ícone do Ubuntu); uma entrada com `updates`
   completa o perfil daquele `guid`. O que está no `settings.json` vence o fragmento. Um perfil que só
   existe em fragmento entra no fim.
5. Perfis com `hidden: true` (no resultado final) **não existem** para o Zefiro: não aparecem nem em
   "Ocultos". São os que o próprio Terminal esconde (no PC do usuário, a cópia antiga do Ubuntu).
6. A ordem base é a do `settings.json`.

O Zefiro **nunca grava** nos arquivos do Windows Terminal.

### Ícones

Em ordem, até um carregar:

1. Campo `icon` do perfil, com variáveis de ambiente expandidas:
   - caminho de arquivo (`.png`, `.ico`, `.jpg`) → o arquivo;
   - `ms-appx:///<caminho>.png` (ex.: `ms-appx:///ProfileIcons/pwsh.png`) → `<pasta do pacote do
     Terminal>\<caminho>.scale-200.png` (e `.scale-100.png`, depois sem sufixo);
   - qualquer outra coisa (emoji, glifo) → passa adiante.
2. `<pasta do pacote>\ProfileIcons\{guid}.scale-200.png` — é assim que vêm os ícones de PowerShell,
   Windows PowerShell, Prompt de comando e Azure. Conferido no PC do usuário: a pasta é legível.
3. O glifo de terminal (`E756`) na cor secundária do tema.

Decodificação por WIC em `platform-windows`, na mesma thread da leitura, já no tamanho de 36 px
(18 DIP a 200%). A pasta do pacote vem da API de pacotes do Windows (`PackageManager`/`Package.InstalledLocation`).

### Abrir um perfil

`wt.exe -p "{guid}"`, desanexado (`system::spawn_detached`), com o `wt.exe` do alias em
`%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe`. Janela nova ou aba nova fica a cargo da configuração
do próprio Terminal (`windowingBehavior`). Se falhar, aviso no log; o painel fecha do mesmo jeito.

## Onde a escolha fica

No `config.toml`, numa seção nova, gravada com `toml_edit` (preserva comentários), como o resto:

```toml
[terminal]
order  = ["{574e775e-…}", "{61c54bbd-…}", "{05554e26-…}", "{0caa0dad-…}"]  # visíveis, nesta ordem
hidden = ["{b453ae62-4e3d-5e58-b989-0a998ec441b8}"]                          # Azure Cloud Shell
```

- Chave por `guid`, não por nome: renomear o perfil no Terminal não perde a escolha. Os perfis
  padrão e os do WSL têm o mesmo `guid` em qualquer PC, então a escolha viaja junto com o
  `config.toml`.
- **Visíveis** = perfis do Terminal fora de `hidden`, ordenados pela posição em `order`; os que não
  estão em `order` (perfil novo, outro PC) vão para o fim, na ordem do Terminal. Ou seja, **perfil
  novo aparece sozinho**.
- Ao gravar: `order` recebe a ordem atual dos visíveis e `hidden` a lista dos ocultos. `guid`s que
  estavam no arquivo e o Terminal não conhece agora (distro desinstalada) **ficam** em `hidden`, para a
  escolha voltar se o perfil voltar. *(chamada minha)*
- Sem a seção, tudo visível na ordem do Terminal (o mesmo que hoje se vê nas "Tarefas").
- A gravação passa pela mesma conferência de revisão do modo de edição (`config_worker`): se o arquivo
  mudou por fora desde a leitura, relê, reaplica só `[terminal]` e grava. O recarregamento ao salvar
  já existente atualiza todos os painéis.

## Onde mora cada parte

| Parte | Crate |
| --- | --- |
| Ler `settings.json` e fragmentos (texto → perfis), JSONC, ícone como "fonte", aplicar `order`/`hidden`, argumentos do `wt` | `crates/integrations/terminal` (nova, portável, com testes) |
| Seção `[terminal]` no `Config` e a gravação | `crates/core` |
| Desenho dos dois modos, áreas de clique, regras de arraste e teclado (estado da edição puro) | `crates/modules/bar/src/terminal.rs` (portável, com testes) |
| Pasta do pacote, leitura de arquivo, WIC dos ícones | `crates/platform-windows` |
| Catálogo, glifo do chip, `PanelKind::Terminal`, thread de leitura, cliques/arraste/teclas, gravação | `crates/modules/bar/src/catalog.rs`, `crates/app` (`terminal_loop.rs`, como o `vscode_loop.rs`) |

A regra da crate `windows` continua: nada de `windows` fora de `platform-windows` e `app`.

## Não objetivos

- Editar os perfis do Terminal (nome, ícone, comando) ou gravar no `settings.json` dele.
- "Executar como administrador", escolher aba nova vs. janela nova, abrir numa pasta.
- Busca no painel.
- Editar os perfis pelo "Editar barra" (desenho A, descartado na conversa).
- Outros terminais (Alacritty, WezTerm…) e perfis do `cmd`/PowerShell fora do Windows Terminal.

## Anti-objetivos

- Abrir o perfil errado ou abrir um perfil ao clicar no modo de edição.
- O painel andar sob o cursor ao entrar/sair da edição (ele cresce a partir da âncora, ADR-0010).
- Perder a escolha do usuário: um perfil que some e volta mantém o "oculto".
- Ler arquivos ou decodificar ícones na thread de UI.

## Verificação

- Testes da integração com o `settings.json` e o fragmento do WSL reais do usuário (copiados como
  fixture, sem dados sensíveis): sai PowerShell, Windows PowerShell, Prompt de comando, Azure Cloud
  Shell, Ubuntu-24.04; a cópia antiga do Ubuntu (`hidden` por `updates`) não sai; o padrão é o Ubuntu;
  JSONC com comentário e vírgula final funciona.
- Testes de `order`/`hidden`: perfil novo vai para o fim; `guid` desconhecido em `hidden` sobrevive à
  gravação; esconder todos.
- Testes do estado de edição: arrastar para "Ocultos" esconde, olho mostra no fim, Esc descarta,
  fechar grava.
- Na tela: abrir o painel, abrir cada perfil, editar (arrastar e olho), concluir, reabrir e ver a
  ordem; reiniciar a barra e ver que ficou.

## O que fica com quem constrói

Medidas exatas (altura das linhas, largura do painel, tamanho do lápis), o desenho do olho (glifos
`E7B3`/`ED1A` da Segoe Fluent ou outro), a posição no catálogo e se a alça é só um desenho ou a única
área de arraste.
