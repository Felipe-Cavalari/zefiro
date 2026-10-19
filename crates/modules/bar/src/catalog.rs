//! Catálogo local de itens implementados; a ilha fica no centro do monitor principal.

pub struct ItemDescriptor { pub id: &'static str, pub name: &'static str, pub description: &'static str }

pub const ITEMS: &[ItemDescriptor] = &[
    ItemDescriptor { id: "zefiro", name: "Zefiro", description: "Abre o menu da barra" },
    ItemDescriptor { id: "clock", name: "Relógio e calendário", description: "Data, hora e calendário do mês" },
    ItemDescriptor { id: "audio", name: "Som", description: "Volume e mudo; painel com a saída de som" },
    ItemDescriptor { id: "mic", name: "Microfone", description: "Mudo e volume; painel com a entrada de som" },
    ItemDescriptor { id: "brightness", name: "Brilho", description: "Brilho dos monitores por DDC/CI; a roda ajusta" },
    ItemDescriptor { id: "claude", name: "Claude", description: "Uso do Claude (5 h e 7 dias)" },
    ItemDescriptor { id: "codex", name: "Codex", description: "Uso do Codex (janelas de uso)" },
    ItemDescriptor { id: "vscode", name: "VS Code", description: "Projetos recentes do VS Code" },
    ItemDescriptor { id: "notifications", name: "Notificações", description: "Contador; clique abre a central" },
    ItemDescriptor { id: "dnd", name: "Não perturbe", description: "Liga e desliga o Não perturbe; o botão do meio abre as configurações" },
    ItemDescriptor { id: "power", name: "Energia", description: "Bloquear, suspender, sair, reiniciar e desligar" },
];

pub fn descriptor(id: &str) -> Option<&'static ItemDescriptor> { ITEMS.iter().find(|item| item.id == id) }

pub fn name(id: &str) -> &str { descriptor(id).map_or("Item indisponível", |item| item.name) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_items_are_in_the_catalog() {
        for id in ["claude", "codex", "vscode", "notifications", "dnd", "power"] {
            assert!(descriptor(id).is_some(), "{id} fora do catálogo");
        }
        let names: std::collections::HashSet<_> = ITEMS.iter().map(|i| i.name).collect();
        assert_eq!(names.len(), ITEMS.len(), "nomes repetidos");
        assert!(ITEMS.iter().all(|i| !i.name.is_empty() && !i.description.is_empty()));
    }
}
