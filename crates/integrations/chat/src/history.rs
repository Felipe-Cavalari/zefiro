//! Conversas antigas: uma por arquivo (`<pasta>\<id>.json`), a lista para o painel e a conversa
//! única da primeira versão (`conversa.json`), que vira um arquivo destes.

use crate::agent::Agent;
use crate::conversation::{Conversation, Role};
use std::path::{Path, PathBuf};

/// Uma linha da lista de conversas antigas.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary { pub id: String, pub agent: Agent, pub origin: String, pub title: String, pub updated: i64 }

/// Só letras, números e `-` no nome do arquivo.
fn safe(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn file(dir: &Path, id: &str) -> Option<PathBuf> {
    safe(id).then(|| dir.join(format!("{id}.json")))
}

/// Grava a conversa (arquivo temporário e troca). Conversa sem mensagens não é gravada.
pub fn save(dir: &Path, c: &Conversation) -> std::io::Result<()> {
    let path = file(dir, &c.id).ok_or_else(|| std::io::Error::other("id de conversa inválido"))?;
    if c.messages.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, c.to_json())?;
    std::fs::rename(&tmp, path)
}

pub fn load(dir: &Path, id: &str) -> Option<Conversation> {
    let text = std::fs::read_to_string(file(dir, id)?).ok()?;
    Conversation::from_json(&text).filter(|c| c.id == id)
}

/// Apaga a cópia do Zefiro e as imagens dela em `images` (a sessão do `claude`/`codex` fica onde
/// está).
pub fn delete(dir: &Path, images: &Path, id: &str) -> std::io::Result<()> {
    if let Some(c) = load(dir, id) {
        for img in c.images() {
            if let Some(path) = image_file(images, img) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    match file(dir, id).map(std::fs::remove_file) {
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// O PNG de uma imagem colada.
pub fn image_file(images: &Path, id: &str) -> Option<PathBuf> {
    safe(id).then(|| images.join(format!("{id}.png")))
}

/// As conversas com alguma pergunta, da mais recente para a mais antiga.
pub fn list(dir: &Path) -> Vec<Summary> {
    let mut out: Vec<Summary> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| Conversation::from_json(&t))
        .filter(|c| !c.messages.is_empty() && safe(&c.id))
        .map(|c| Summary { title: c.title(), id: c.id, agent: c.agent, origin: c.origin, updated: c.updated })
        .collect();
    out.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Uma imagem da biblioteca: o id do PNG, a conversa de onde veio e se o agente a gerou (na
/// resposta) ou o usuário a colou (na pergunta).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryImage { pub id: String, pub conversation: String, pub generated: bool }

/// As imagens de todas as conversas, da conversa mais recente para a mais antiga e, dentro dela,
/// da última mensagem para a primeira. Imagem cujo arquivo sumiu fica de fora.
pub fn library(dir: &Path, images: &Path) -> Vec<LibraryImage> {
    let mut out = Vec::new();
    for summary in list(dir) {
        let Some(c) = load(dir, &summary.id) else { continue };
        for m in c.messages.iter().rev() {
            for id in m.images.iter().rev() {
                if image_file(images, id).is_some_and(|p| p.is_file()) {
                    out.push(LibraryImage { id: id.clone(), conversation: c.id.clone(), generated: m.role == Role::Claude });
                }
            }
        }
    }
    out
}

/// A conversa mais recente (a que o painel mostra ao abrir pela primeira vez).
pub fn latest(dir: &Path) -> Option<Conversation> {
    list(dir).first().and_then(|s| load(dir, &s.id))
}

/// Leva o `conversa.json` da primeira versão para a pasta das conversas e o apaga.
pub fn migrate(legacy: &Path, dir: &Path) {
    let Some(c) = std::fs::read_to_string(legacy).ok().and_then(|t| Conversation::from_json(&t)) else { return };
    if save(dir, &c).is_ok() {
        let _ = std::fs::remove_file(legacy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(id: &str, agent: Agent, question: &str, updated: i64) -> Conversation {
        let mut c = Conversation::new(id.into(), agent);
        c.origin = "Windows".into();
        c.updated = updated;
        if !question.is_empty() {
            c.ask(question);
        }
        c
    }

    #[test]
    fn save_list_load_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("conversas");
        save(&d, &conv("a-1", Agent::Claude, "Primeira pergunta\nsegunda linha", 10)).unwrap();
        save(&d, &conv("b-2", Agent::Codex, "Outra", 20)).unwrap();
        save(&d, &conv("c-3", Agent::Codex, "", 30)).unwrap();
        let l = list(&d);
        assert_eq!(l.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["b-2", "a-1"], "mais recente primeiro, sem as vazias");
        assert_eq!((l[1].title.as_str(), l[1].agent, l[1].origin.as_str()), ("Primeira pergunta", Agent::Claude, "Windows"));
        assert_eq!(load(&d, "a-1").unwrap().messages.len(), 2);
        assert_eq!(latest(&d).unwrap().id, "b-2");
        let imgs = dir.path().join("imagens");
        std::fs::create_dir_all(&imgs).unwrap();
        let mut with_image = conv("d-4", Agent::Claude, "", 5);
        with_image.ask_with("veja", vec!["img-1".into()]);
        save(&d, &with_image).unwrap();
        std::fs::write(image_file(&imgs, "img-1").unwrap(), b"png").unwrap();
        delete(&d, &imgs, "d-4").unwrap();
        assert!(!imgs.join("img-1.png").exists(), "as imagens vão junto");
        delete(&d, &imgs, "b-2").unwrap();
        delete(&d, &imgs, "nao-existe").unwrap();
        assert_eq!(list(&d).len(), 1);
    }

    #[test]
    fn library_lists_images_newest_first_with_their_origin() {
        let dir = tempfile::tempdir().unwrap();
        let (d, imgs) = (dir.path().join("conversas"), dir.path().join("imagens"));
        std::fs::create_dir_all(&imgs).unwrap();
        for id in ["p1", "g1", "p2", "g2", "g3"] {
            std::fs::write(image_file(&imgs, id).unwrap(), b"png").unwrap();
        }
        let mut old = conv("old", Agent::Codex, "", 10);
        old.ask_with("veja", vec!["p1".into()]);
        old.add_reply_image("g1".into());
        save(&d, &old).unwrap();
        let mut new = conv("new", Agent::Codex, "", 20);
        new.ask_with("um gato", vec!["p2".into(), "sumiu".into()]);
        new.add_reply_image("g2".into());
        new.add_reply_image("g3".into());
        save(&d, &new).unwrap();
        let l = library(&d, &imgs);
        let got: Vec<(&str, &str, bool)> = l.iter().map(|i| (i.id.as_str(), i.conversation.as_str(), i.generated)).collect();
        assert_eq!(got, [("g3", "new", true), ("g2", "new", true), ("p2", "new", false), ("g1", "old", true), ("p1", "old", false)], "a do arquivo sumido fica de fora");
        assert!(library(&dir.path().join("nada"), &imgs).is_empty());
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        let d = Path::new("x");
        assert!(file(d, "../config").is_none());
        assert!(file(d, "").is_none());
        assert!(file(d, r"a\b").is_none());
        assert!(file(d, "01a160e0-6e1c-73d2").is_some());
    }

    #[test]
    fn first_version_file_moves_into_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("conversa.json");
        std::fs::write(&legacy, r#"{"session":"s-1","created":true,"origin":"Windows","messages":[{"role":"User","text":"oi","state":"Done"}]}"#).unwrap();
        let d = dir.path().join("conversas");
        migrate(&legacy, &d);
        assert!(!legacy.exists());
        let c = load(&d, "s-1").unwrap();
        assert_eq!((c.session.as_str(), c.created, c.agent), ("s-1", true, Agent::Claude));
        migrate(&legacy, &d);
    }
}
