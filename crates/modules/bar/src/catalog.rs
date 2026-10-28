//! Catálogo local de itens implementados. A ilha é um chip como os outros (ADR-0017), e o player, o
//! clima e o monitor do PC levam partes dela para qualquer seção.

pub struct ItemDescriptor { pub id: &'static str, pub name: &'static str, pub description: &'static str }

pub const ITEMS: &[ItemDescriptor] = &[
    ItemDescriptor { id: "zefiro", name: "Zefiro", description: "Abre o menu da barra" },
    ItemDescriptor { id: "clock", name: "Relógio e calendário", description: "Data, hora e calendário do mês" },
    ItemDescriptor { id: "audio", name: "Som", description: "Volume e mudo; painel com a saída de som" },
    ItemDescriptor { id: "mic", name: "Microfone", description: "Mudo e volume; painel com a entrada de som" },
    ItemDescriptor { id: "brightness", name: "Brilho", description: "Brilho dos monitores por DDC/CI; a roda ajusta" },
    ItemDescriptor { id: "claude", name: "Claude", description: "Uso do Claude (5 h e 7 dias)" },
    ItemDescriptor { id: "codex", name: "Codex", description: "Uso do Codex (janelas de uso)" },
    ItemDescriptor { id: "grok", name: "Grok", description: "Uso do Grok (cota semanal)" },
    ItemDescriptor { id: "ia", name: "Uso das IAs", description: "Claude, Codex e Grok num chip só, alternando entre as contas" },
    ItemDescriptor { id: "vscode", name: "VS Code", description: "Projetos recentes do VS Code" },
    ItemDescriptor { id: "terminal", name: "Terminal", description: "Perfis do Windows Terminal; o lápis escolhe e ordena" },
    ItemDescriptor { id: "chat", name: "Chat com IA", description: "Pergunte ao Claude, ao Codex ou ao Grok direto da barra" },
    ItemDescriptor { id: "spotify", name: "Spotify", description: "Busque e toque no Spotify sem abrir o app (Premium)" },
    ItemDescriptor { id: "notifications", name: "Notificações", description: "Contador; clique abre a central" },
    ItemDescriptor { id: "dnd", name: "Não perturbe", description: "Liga e desliga o Não perturbe; o botão do meio abre as configurações" },
    ItemDescriptor { id: "island", name: "Ilha", description: "Mídia, avisos e espiada; clique abre o painel com abas" },
    ItemDescriptor { id: "player", name: "Player", description: "A mídia tocando, como na ilha; clique abre os controles" },
    ItemDescriptor { id: "weather", name: "Clima", description: "Temperatura e cidade; clique abre a previsão" },
    ItemDescriptor { id: "hardware", name: "Monitor do PC", description: "CPU, GPU, RAM, disco e rede ao clicar" },
    ItemDescriptor { id: "tray", name: "Bandeja", description: "Ícones da bandeja do sistema; clique abre, botão direito mostra o menu do app" },
    ItemDescriptor { id: "power", name: "Energia", description: "Bloquear, suspender, sair, reiniciar e desligar" },
];

pub fn descriptor(id: &str) -> Option<&'static ItemDescriptor> { ITEMS.iter().find(|item| item.id == id) }

pub fn name(id: &str) -> &str { descriptor(id).map_or("Item indisponível", |item| item.name) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_items_are_in_the_catalog() {
        for id in ["claude", "codex", "grok", "ia", "vscode", "terminal", "chat", "notifications", "dnd", "power", "player", "weather", "hardware", "island", "tray", "spotify"] {
            assert!(descriptor(id).is_some(), "{id} fora do catálogo");
        }
        let names: std::collections::HashSet<_> = ITEMS.iter().map(|i| i.name).collect();
        assert_eq!(names.len(), ITEMS.len(), "nomes repetidos");
        assert!(ITEMS.iter().all(|i| !i.name.is_empty() && !i.description.is_empty()));
        assert_eq!(descriptor("spotify").map(|d| (d.name, d.description)), Some(("Spotify", "Busque e toque no Spotify sem abrir o app (Premium)")));
    }

    #[test]
    fn chat_is_named_for_the_three_agents() {
        // O chat fala com o Claude, o Codex ou o Grok (`[chat] agent`).
        assert_eq!(descriptor("chat").map(|d| (d.name, d.description)), Some(("Chat com IA", "Pergunte ao Claude, ao Codex ou ao Grok direto da barra")));
        assert_eq!(descriptor("grok").map(|d| (d.name, d.description)), Some(("Grok", "Uso do Grok (cota semanal)")));
    }
}
