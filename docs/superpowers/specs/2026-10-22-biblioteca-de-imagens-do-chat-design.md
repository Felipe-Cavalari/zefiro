# Biblioteca de imagens do chat — design

Data: 2026-10-22. Pedido: uma "mini biblioteca" das imagens geradas no chat da barra. O usuário
aprovou a proposta ("sim, pode fazer como você descreveu") e pediu para implementar depois desta
spec curta.

## Intenção

Achar de novo uma imagem que o Codex gerou (ou que você colou) sem lembrar em que conversa ela
estava: abrir, mostrar a pasta ou voltar à conversa de onde ela veio.

## Metas

- Um botão de imagem no cabeçalho do chat, ao lado de "Conversas antigas", abre a página
  **Imagens** (como as outras páginas: título com a seta de voltar; Esc volta ao chat).
- Grade de 3 colunas com as imagens de todas as conversas do chat, geradas e coladas, da conversa
  mais recente para a mais antiga e, dentro dela, da última mensagem para a primeira.
- Cada quadro tem a etiqueta "Gerada" (na resposta) ou "Colada" (na pergunta).
- Clique no quadro abre a imagem no app padrão do Windows (e fecha o painel, como no balão).
- Com o mouse em cima, um botão leva à conversa de origem (reabre a conversa, como em
  "Conversas antigas").
- "Abrir pasta" no título abre `%LOCALAPPDATA%\zefiro\chat\imagens` no Explorer.
- A roda rola a grade de uma fileira em uma fileira; 3 fileiras à vista.
- Sem imagens: "Nenhuma imagem ainda."

## Não metas

- Apagar uma imagem sozinha (some ao apagar a conversa, como hoje).
- Busca, filtros, renomear, copiar para a área de transferência, arrastar para fora.
- Imagens do Codex geradas fora do chat da barra (`~/.codex/generated_images`).

## Decisões

- **Sem índice novo:** a lista sai das próprias conversas (`chat\conversas\*.json`) ao abrir a
  página (`history::images`). Uma imagem cujo arquivo sumiu fica de fora.
- **Miniaturas só do que está à vista:** cada quadro é decodificado numa thread, só para as
  fileiras visíveis e a próxima, a 256 px. A mesma memória das imagens do balão é usada, guardando
  o lado em que cada uma foi lida; uma miniatura menor que a pedida é lida de novo.
- **Portável:** a lista (`integrations/chat`) e o desenho da página (`modules/bar/chat.rs`) não
  conhecem a crate `windows`; o laço (`app/chat_loop.rs`) lê os arquivos e abre o visualizador.

## Verificação

- Testes: a ordem e a origem das imagens na lista; a grade (colunas, rolagem, etiquetas, botões e o
  que ganha o clique); o botão no cabeçalho.
- Na barra: abrir a página com as imagens do gato e do bonsai, abrir uma, ir à conversa e abrir a
  pasta.
