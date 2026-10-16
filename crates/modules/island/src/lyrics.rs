//! Letras sincronizadas do LRCLIB (https://lrclib.net), como a ilha C++: busca exata, depois busca
//! tolerante com pontuação; leitura do LRC; linha atual com 0,49 s de antecedência; cache em disco.
//! Portável: endereços, leitura do JSON, pontuação e o cálculo da linha. O HTTP fica na plataforma.

use serde_json::Value;

/// Antecedência base: a linha aparece um pouco antes de ser cantada.
pub const LEAD_MS: i64 = 490;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line { pub at_ms: u32, pub text: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    /// Sincronizada (com tempos) ou só o texto.
    pub synced: bool,
    pub lines: Vec<Line>,
}

/// `[mm:ss.xx] texto` (vários tempos por linha valem; `[offset:+/-ms]` desloca tudo).
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut offset: i64 = 0;
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            rest = &stripped[end + 1..];
            if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
                continue;
            }
            let Some((m, s)) = tag.split_once(':') else { continue };
            let (Ok(m), Ok(s)) = (m.trim().parse::<u32>(), s.trim().parse::<f64>()) else { continue };
            times.push(m as i64 * 60_000 + (s * 1000.0).round() as i64);
        }
        for t in times {
            out.push(Line { at_ms: (t - offset).max(0) as u32, text: rest.trim().to_string() });
        }
    }
    out.sort_by_key(|l| l.at_ms);
    out
}

pub fn parse_plain(text: &str) -> Vec<Line> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(|t| Line { at_ms: 0, text: t.to_string() }).collect()
}

/// Título sem "(feat. …)", "[…]" e sufixos de versão ("- Remastered 2011").
pub fn clean_title(title: &str) -> String {
    let mut t = title.to_string();
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let (Some(a), Some(b)) = (t.find(open), t.find(close)) {
            if b <= a {
                break;
            }
            t.replace_range(a..=b, "");
        }
    }
    if let Some(i) = t.find(" - ") {
        let tail = t[i + 3..].to_lowercase();
        if ["remaster", "live", "version", "versão", "edit", "mix", "ao vivo", "acoustic"].iter().any(|k| tail.contains(k)) {
            t.truncate(i);
        }
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Só o artista principal ("A, B & C feat. D" → "A").
pub fn primary_artist(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let cut = [",", " & ", " feat", " ft.", " x ", ";", " e "].iter().filter_map(|sep| lower.find(sep)).min().unwrap_or(artist.len());
    artist[..cut].trim().to_string()
}

fn normalize(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// 2 = igual, 1 = um contém o outro, 0 = diferente (comparando só letras e números).
pub fn text_match(a: &str, b: &str) -> u8 {
    let (a, b) = (normalize(a), normalize(b));
    if a.is_empty() || b.is_empty() {
        0
    } else if a == b {
        2
    } else if a.contains(&b) || b.contains(&a) {
        1
    } else {
        0
    }
}

pub fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

const BASE: &str = "https://lrclib.net";

/// Consultas em ordem: exata, exata limpa, busca livre, busca só pelo título. `trusted` = exata.
pub fn requests(title: &str, artist: &str, album: &str, duration_s: u32) -> Vec<(String, bool)> {
    let (ct, pa) = (clean_title(title), primary_artist(artist));
    let (st, sa) = (if ct.is_empty() { title.to_string() } else { ct }, if pa.is_empty() { artist.to_string() } else { pa });
    let dur = if duration_s > 0 { format!("&duration={duration_s}") } else { String::new() };
    let alb = if album.is_empty() { String::new() } else { format!("&album_name={}", encode(album)) };
    let mut out = vec![(format!("{BASE}/api/get?track_name={}&artist_name={}{alb}{dur}", encode(title), encode(artist)), true)];
    if st != title || sa != artist {
        out.push((format!("{BASE}/api/get?track_name={}&artist_name={}{dur}", encode(&st), encode(&sa)), true));
    }
    out.push((format!("{BASE}/api/search?q={}", encode(&format!("{st} {sa}"))), false));
    out.push((format!("{BASE}/api/search?track_name={}", encode(&st)), false));
    out
}

#[derive(Debug, Clone, Default)]
pub struct Pick { pub score: i32, pub lyrics: Option<Lyrics> }

impl Pick {
    pub fn synced(&self) -> bool {
        self.lyrics.as_ref().is_some_and(|l| l.synced)
    }
}

/// Pontua cada candidato da resposta (objeto ou lista) e guarda o melhor em `best`.
pub fn consider(body: &str, title: &str, artist: &str, duration_s: u32, trusted: bool, best: &mut Pick) {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return };
    let items: Vec<&Value> = match &v {
        Value::Array(a) => a.iter().collect(),
        other => vec![other],
    };
    let (want_title, want_artist) = (clean_title(title), primary_artist(artist));
    for c in items {
        if c.get("instrumental").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let synced_text = c.get("syncedLyrics").and_then(Value::as_str).unwrap_or("");
        let plain_text = c.get("plainLyrics").and_then(Value::as_str).unwrap_or("");
        let synced_lines = parse_lrc(synced_text);
        let (synced, lines) = if synced_lines.is_empty() { (false, parse_plain(plain_text)) } else { (true, synced_lines) };
        if lines.is_empty() {
            continue;
        }
        let dur_diff = c.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0 && duration_s > 0).map(|d| (d.round() as i64 - duration_s as i64).abs());
        let mut score = 0;
        if !trusted {
            let tm = text_match(c.get("trackName").and_then(Value::as_str).unwrap_or(""), &want_title);
            if tm == 0 || dur_diff.is_some_and(|d| d > 10) {
                continue;
            }
            let am = if want_artist.is_empty() { 1 } else { text_match(c.get("artistName").and_then(Value::as_str).unwrap_or(""), &want_artist) };
            if am == 0 && !dur_diff.is_some_and(|d| d <= 3) {
                continue;
            }
            score += tm as i32 * 30 + am as i32 * 20;
        }
        score += match dur_diff {
            Some(d) if d <= 2 => 40,
            Some(d) if d <= 5 => 25,
            Some(_) => 5,
            None => 0,
        };
        if synced {
            score += 20;
        }
        if best.lyrics.is_none() || score > best.score {
            *best = Pick { score, lyrics: Some(Lyrics { synced, lines }) };
        }
    }
}

/// Linha atual em `position_ms` (já com a antecedência e o deslocamento do usuário).
pub fn current_line(lines: &[Line], position_ms: i64) -> Option<usize> {
    let t = position_ms + LEAD_MS;
    if t < 0 {
        return None;
    }
    lines.iter().rposition(|l| l.at_ms as i64 <= t)
}

/// Quanto da linha `i` já foi cantado (0 → 1), para o karaokê.
pub fn line_progress(lines: &[Line], i: usize, position_ms: i64) -> f32 {
    let t = position_ms + LEAD_MS;
    let start = lines[i].at_ms as i64;
    let end = lines.get(i + 1).map_or(start + 4000, |n| n.at_ms as i64).max(start + 1);
    ((t - start) as f32 / (end - start) as f32).clamp(0.0, 1.0)
}

/// Nome do arquivo do cache para a faixa.
pub fn cache_key(artist: &str, title: &str, duration_s: u32) -> String {
    // FNV-1a de 64 bits: estável entre versões, sem dependência.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{}|{}|{duration_s}", normalize(artist), normalize(title)).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}.lrc")
}

/// Texto do cache: LRC (sincronizada) ou `[plain]` + linhas.
pub fn to_cache(l: &Lyrics) -> String {
    if l.synced {
        l.lines.iter().map(|x| format!("[{:02}:{:02}.{:02}]{}\n", x.at_ms / 60_000, x.at_ms / 1000 % 60, x.at_ms % 1000 / 10, x.text)).collect()
    } else {
        format!("[plain]\n{}", l.lines.iter().map(|x| format!("{}\n", x.text)).collect::<String>())
    }
}

pub fn from_cache(text: &str) -> Option<Lyrics> {
    match text.strip_prefix("[plain]\n") {
        Some(rest) => Some(Lyrics { synced: false, lines: parse_plain(rest) }),
        None => {
            let lines = parse_lrc(text);
            (!lines.is_empty()).then_some(Lyrics { synced: true, lines })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_offset_and_repeated_times() {
        let l = parse_lrc("[ti:x]\n[offset:+500]\n[00:12.50]Primeira\n[00:20.00][01:00.00]Refrão\n[00:30.00]\n");
        assert_eq!(l.iter().map(|x| (x.at_ms, x.text.as_str())).collect::<Vec<_>>(), vec![(12_000, "Primeira"), (19_500, "Refrão"), (29_500, ""), (59_500, "Refrão")]);
    }

    #[test]
    fn current_line_uses_the_lead_and_progress_sweeps() {
        let l = parse_lrc("[00:10.00]a\n[00:14.00]b\n");
        assert_eq!(current_line(&l, 9_000), None);
        assert_eq!(current_line(&l, 9_600), Some(0), "0,49 s antes");
        assert_eq!(current_line(&l, 13_600), Some(1));
        assert!((line_progress(&l, 0, 11_510) - 0.5).abs() < 0.01);
        assert_eq!(line_progress(&l, 1, 100_000), 1.0);
    }

    #[test]
    fn cleans_titles_and_artists() {
        assert_eq!(clean_title("Song (feat. X) [Live] - Remastered 2011"), "Song");
        assert_eq!(clean_title("Hey - You"), "Hey - You");
        assert_eq!(primary_artist("Anitta, J Balvin & X feat. Y"), "Anitta");
        assert_eq!(text_match("Don't Stop", "dont stop"), 2);
        assert_eq!(text_match("Song (Live)", "song"), 1);
        assert_eq!(text_match("a", "b"), 0);
        assert_eq!(encode("São Paulo & co"), "S%C3%A3o%20Paulo%20%26%20co");
    }

    #[test]
    fn requests_go_exact_then_search() {
        let r = requests("Song (feat. X)", "A, B", "Álbum", 200);
        assert!(r[0].0.contains("/api/get?track_name=Song%20%28feat.%20X%29&artist_name=A%2C%20B&album_name=%C3%81lbum&duration=200") && r[0].1);
        assert!(r[1].0.ends_with("/api/get?track_name=Song&artist_name=A&duration=200"));
        assert!(r[2].0.ends_with("/api/search?q=Song%20A") && !r[2].1);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn scoring_prefers_synced_matching_duration_and_rejects_other_songs() {
        let body = r#"[
          {"trackName":"Song","artistName":"A","duration":250,"plainLyrics":"x","syncedLyrics":"[00:01.00]longe"},
          {"trackName":"Song","artistName":"A","duration":201,"plainLyrics":"só texto"},
          {"trackName":"Song","artistName":"A","duration":199,"syncedLyrics":"[00:01.00]certa"},
          {"trackName":"Outra","artistName":"A","duration":200,"syncedLyrics":"[00:01.00]outra"},
          {"trackName":"Song","artistName":"A","instrumental":true}
        ]"#;
        let mut best = Pick::default();
        consider(body, "Song", "A", 200, false, &mut best);
        assert!(best.synced());
        assert_eq!(best.lyrics.unwrap().lines[0].text, "certa");
    }

    #[test]
    fn cache_round_trip_and_stable_key() {
        let synced = Lyrics { synced: true, lines: parse_lrc("[01:02.34]oi\n") };
        assert_eq!(from_cache(&to_cache(&synced)), Some(synced));
        let plain = Lyrics { synced: false, lines: parse_plain("a\n\nb") };
        assert_eq!(from_cache(&to_cache(&plain)), Some(plain));
        assert_eq!(cache_key("A", "Song!", 200), cache_key("a", "song", 200));
        assert_ne!(cache_key("A", "Song", 200), cache_key("A", "Song", 201));
    }
}
