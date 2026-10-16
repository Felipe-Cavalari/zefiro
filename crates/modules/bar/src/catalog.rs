//! Catálogo local de itens implementados; a ilha fica no centro do monitor principal.

pub struct ItemDescriptor { pub id: &'static str, pub name: &'static str, pub description: &'static str }

pub const ITEMS: &[ItemDescriptor] = &[
    ItemDescriptor { id: "zefiro", name: "Zefiro", description: "Abre o menu da barra" },
    ItemDescriptor { id: "clock", name: "Relógio e calendário", description: "Data, hora e calendário do mês" },
    ItemDescriptor { id: "audio", name: "Som", description: "Volume e mudo; painel com a saída de som" },
    ItemDescriptor { id: "mic", name: "Microfone", description: "Mudo e volume; painel com a entrada de som" },
    ItemDescriptor { id: "brightness", name: "Brilho", description: "Brilho dos monitores por DDC/CI; a roda ajusta" },
];

pub fn descriptor(id: &str) -> Option<&'static ItemDescriptor> { ITEMS.iter().find(|item| item.id == id) }

pub fn name(id: &str) -> &str { descriptor(id).map_or("Item indisponível", |item| item.name) }
