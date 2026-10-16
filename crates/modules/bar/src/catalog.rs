//! Catálogo local de itens implementados; a ilha é gerida pelo host separado.

pub struct ItemDescriptor { pub id: &'static str, pub name: &'static str, pub description: &'static str }

pub const ITEMS: &[ItemDescriptor] = &[
    ItemDescriptor { id: "zefiro", name: "Zefiro", description: "Abre o menu da barra" },
    ItemDescriptor { id: "clock", name: "Relógio e calendário", description: "Data, hora e calendário do mês" },
];

pub fn descriptor(id: &str) -> Option<&'static ItemDescriptor> { ITEMS.iter().find(|item| item.id == id) }

pub fn name(id: &str) -> &str { descriptor(id).map_or("Item indisponível", |item| item.name) }
