//! Sonda: busca a letra de uma música no LRCLIB (sem cache). Uso: -- "Artista" "Título" [segundos]
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (artist, title) = (a.first().cloned().unwrap_or("Queen".into()), a.get(1).cloned().unwrap_or("Bohemian Rhapsody".into()));
    let dur: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(355);
    let mut best = zefiro_island::lyrics::Pick::default();
    for (url, trusted) in zefiro_island::lyrics::requests(&title, &artist, "", dur) {
        let r = zefiro_platform_windows::http::get(&url);
        println!("{} → {:?}", url, r.as_ref().map(|(s, b)| (*s, b.len())));
        if let Ok((200, body)) = r { zefiro_island::lyrics::consider(&body, &title, &artist, dur, trusted, &mut best); }
        if best.synced() { break; }
    }
    if let Some(l) = best.lyrics { println!("sincronizada={} linhas={} primeira={:?}", l.synced, l.lines.len(), l.lines.iter().find(|x| !x.text.is_empty())); }
}
