# ADR-0015: Chip Terminal e edição dos perfis no próprio painel

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main (commits 9e40327..f9d4c87)

## Contexto

O usuário abre o Windows Terminal pela lista de atalhos da barra de tarefas e quer o mesmo na barra
do Zefiro, escolhendo quais perfis aparecem e em que ordem, pela interface
(`docs/superpowers/specs/2026-10-20-chip-terminal-design.md`). Era a primeira configuração que não
caberia no "Editar barra": ela depende de dados que só existem ao abrir o painel (os perfis do
Terminal).

## Opções consideradas

1. **Página "Opções do Terminal" no "Editar barra"**, como as opções do relógio: coerente com o resto,
   mas obriga a abrir o modo de edição para um ajuste que se faz olhando os perfis.
2. **Edição no próprio painel** (lápis → arrastar e olho → Concluir): ajuste na hora, onde os perfis
   estão. Escolhida nos mockups.
3. Para os perfis: ler a lista de atalhos do Windows (arquivos binários de `AutomaticDestinations`) ou
   **ler o `settings.json` e os fragmentos do Terminal**. A segunda é texto documentado.

## Decisão

Edição dentro do painel, com os perfis lidos do `settings.json` (Stable, Preview ou sem pacote) e dos
fragmentos, numa thread ao abrir o painel. O Zefiro nunca grava nos arquivos do Terminal. A escolha
fica em `[terminal] order` e `hidden` no `config.toml`, por `guid` (os perfis padrão e os do WSL têm
o mesmo `guid` em qualquer PC). Perfis com `hidden: true` e os de fontes em
`disabledProfileSources` não existem para o Zefiro, como para o Terminal (é por isso que o Azure
Cloud Shell some quando a fonte `Windows.Terminal.Azure` está desativada).

## Consequências

- **Recarga do config sem fechar o painel:** qualquer mudança no `config.toml` fechava o painel
  aberto. Quando só `[terminal]` muda (`Config::same_except_terminal`), a barra só atualiza; senão
  "Concluir" fecharia o painel na cara do usuário.
- **Gravação direta:** `config::set_terminal` reescreve só a tabela `[terminal]` com `toml_edit`,
  lendo o arquivo atual, como o menu do Z faz com tema e altura; não passa pela conferência de
  conflitos do "Editar barra" (que não toca em `[terminal]`).
- **`wt.exe` é um alias de execução:** `Path::exists` dá `false` nele; `windows_terminal::wt_exe`
  confere com `symlink_metadata`.
- **Ícones:** `ms-appx:///…` vira a pasta do pacote (`GetPackagesByPackageFamily` +
  `GetPackagePathByFullName`) com as variantes `.scale-200`/`.scale-100`; os perfis padrão usam
  `ProfileIcons\{guid}`; o resto cai no glifo `E756`. A decodificação (WIC) roda na thread de leitura,
  que inicia o WinRT só durante a chamada.
- Fica aberto: edição de outros painéis no mesmo molde, se aparecer outro caso.
