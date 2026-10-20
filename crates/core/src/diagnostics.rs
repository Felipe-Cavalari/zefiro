//! Exportar diagnóstico: limpeza de dados pessoais e um gravador de `.zip` mínimo (sem compressão),
//! para não puxar dependência nova só para isso.

/// CRC-32 (IEEE 802.3), o do formato zip.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Data e hora no formato do DOS (o do zip): `(hora, data)`. Segundos de 2 em 2.
pub fn dos_datetime(year: u16, month: u16, day: u16, hour: u16, minute: u16, second: u16) -> (u16, u16) {
    ((hour << 11) | (minute << 5) | (second / 2), (year.saturating_sub(1980) << 9) | (month << 5) | day)
}

/// Zip só com arquivos guardados (método 0), nomes em UTF-8.
pub struct ZipWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
    time: u16,
    date: u16,
}

/// "Versão necessária" 2.0 e a marca de nome em UTF-8 (bit 11).
const VERSION: u16 = 20;
const UTF8: u16 = 1 << 11;

impl ZipWriter {
    pub fn new((time, date): (u16, u16)) -> ZipWriter {
        ZipWriter { out: Vec::new(), central: Vec::new(), count: 0, time, date }
    }

    /// Acrescenta um arquivo. Logs e config cabem com folga nos limites do zip sem ZIP64 (4 GB).
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let offset = self.out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name = name.as_bytes();
        let put16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
        let put32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        let o = &mut self.out;
        put32(o, 0x0403_4B50);
        put16(o, VERSION);
        put16(o, UTF8);
        put16(o, 0);
        put16(o, self.time);
        put16(o, self.date);
        put32(o, crc);
        put32(o, size);
        put32(o, size);
        put16(o, name.len() as u16);
        put16(o, 0);
        o.extend_from_slice(name);
        o.extend_from_slice(data);
        let c = &mut self.central;
        put32(c, 0x0201_4B50);
        put16(c, VERSION);
        put16(c, VERSION);
        put16(c, UTF8);
        put16(c, 0);
        put16(c, self.time);
        put16(c, self.date);
        put32(c, crc);
        put32(c, size);
        put32(c, size);
        put16(c, name.len() as u16);
        put16(c, 0);
        put16(c, 0);
        put16(c, 0);
        put16(c, 0);
        put32(c, 0);
        put32(c, offset);
        c.extend_from_slice(name);
        self.count += 1;
    }

    pub fn finish(mut self) -> Vec<u8> {
        let start = self.out.len() as u32;
        let size = self.central.len() as u32;
        self.out.append(&mut self.central);
        let o = &mut self.out;
        o.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
        o.extend_from_slice(&[0, 0, 0, 0]);
        o.extend_from_slice(&self.count.to_le_bytes());
        o.extend_from_slice(&self.count.to_le_bytes());
        o.extend_from_slice(&size.to_le_bytes());
        o.extend_from_slice(&start.to_le_bytes());
        o.extend_from_slice(&[0, 0]);
        self.out
    }
}

/// O `.zip` do exportar diagnóstico: os logs (`zefiro*.log`) e relatórios de crash (`crash-*.txt`)
/// da pasta de logs, o `config.toml` e o `info.txt`, todos limpos por `sanitize`.
pub fn diagnostic_zip(log_dir: &std::path::Path, config: Option<&str>, info: &str, home: &str, user: &str, when: (u16, u16)) -> Vec<u8> {
    let name_of = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut logs: Vec<std::path::PathBuf> = std::fs::read_dir(log_dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    logs.retain(|p| {
        let name = name_of(p);
        (name.starts_with("zefiro") && name.ends_with(".log")) || (name.starts_with("crash-") && name.ends_with(".txt"))
    });
    logs.sort();
    let mut zip = ZipWriter::new(when);
    for path in logs {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        zip.add(&format!("logs/{}", name_of(&path)), sanitize(&String::from_utf8_lossy(&bytes), home, user).as_bytes());
    }
    if let Some(config) = config {
        zip.add("config.toml", sanitize(config, home, user).as_bytes());
    }
    zip.add("info.txt", sanitize(info, home, user).as_bytes());
    zip.finish()
}

/// Tira do texto o que identifica o usuário: a pasta dele (vira `%USERPROFILE%`), e-mails, o que tem
/// cara de token e o nome de usuário (como palavra inteira, a partir de 3 letras).
pub fn sanitize(text: &str, home: &str, user: &str) -> String {
    let mut s = text.to_string();
    let home = home.trim_end_matches(['\\', '/']);
    if !home.is_empty() {
        for variant in [home.replace('\\', "\\\\"), home.to_string(), home.replace('\\', "/")] {
            s = replace_ascii_case_insensitive(&s, &variant, "%USERPROFILE%");
        }
    }
    s = hide_emails(&s);
    s = hide_tokens(&s);
    if user.chars().count() >= 3 {
        s = replace_word(&s, user, "<usuario>");
    }
    s
}

fn replace_ascii_case_insensitive(text: &str, needle: &str, with: &str) -> String {
    let (lower, pat) = (text.to_ascii_lowercase(), needle.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(&pat) {
        if i < last {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = i + pat.len();
    }
    out.push_str(&text[last..]);
    out
}

fn replace_word(text: &str, word: &str, with: &str) -> String {
    let (lower, pat) = (text.to_lowercase(), word.to_lowercase());
    if lower.len() != text.len() {
        // Minúsculas com outro tamanho em bytes (raro): só a grafia exata.
        return text.replace(word, with);
    }
    let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(&pat) {
        let end = i + pat.len();
        if i < last || !boundary(text[..i].chars().next_back()) || !boundary(text[end..].chars().next()) {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

fn hide_emails(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let local = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let domain = |c: char| c.is_ascii_alphanumeric() || ".-".contains(c);
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i < chars.len() {
        if chars[i] == '@' {
            let mut a = i;
            while a > copied && local(chars[a - 1]) {
                a -= 1;
            }
            let mut b = i + 1;
            while b < chars.len() && domain(chars[b]) {
                b += 1;
            }
            let host: String = chars[i + 1..b].iter().collect();
            let host = host.trim_end_matches('.');
            if a < i && host.contains('.') && !host.starts_with('.') {
                out.extend(&chars[copied..a]);
                out.push_str("<email>");
                copied = i + 1 + host.chars().count();
                i = copied;
                continue;
            }
        }
        i += 1;
    }
    out.extend(&chars[copied..]);
    out
}

fn hide_tokens(text: &str) -> String {
    let token_char = |c: char| c.is_ascii_alphanumeric() || "+/=_-".contains(c);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("Bearer ") {
            let end = after.find(|c: char| c.is_whitespace() || "\"';,".contains(c)).unwrap_or(after.len());
            if end > 0 {
                out.push_str("Bearer <token>");
                rest = &after[end..];
                continue;
            }
        }
        if token_char(c) {
            let end = rest.find(|c: char| !token_char(c)).unwrap_or(rest.len());
            let run = &rest[..end];
            let mixed = run.chars().any(|c| c.is_ascii_digit()) && run.chars().any(|c| c.is_ascii_alphabetic());
            if (run.starts_with("sk-") && run.len() >= 20) || (run.len() >= 32 && mixed) {
                out.push_str("<token>");
            } else {
                out.push_str(run);
            }
            rest = &rest[end..];
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(b: &[u8], i: usize) -> u16 {
        u16::from_le_bytes([b[i], b[i + 1]])
    }

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
    }

    #[test]
    fn crc_of_the_standard_check_string() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn dos_datetime_packs_fields() {
        let (time, date) = dos_datetime(2026, 10, 20, 14, 30, 58);
        assert_eq!(time, (14 << 11) | (30 << 5) | 29);
        assert_eq!(date, ((2026 - 1980) << 9) | (10 << 5) | 20);
    }

    #[test]
    fn zip_has_local_headers_central_directory_and_end_record() {
        let mut z = ZipWriter::new(dos_datetime(2026, 10, 20, 12, 0, 0));
        z.add("logs/zefiro.log", b"ola");
        z.add("config.toml", b"theme = \"cobalto\"\n");
        let b = z.finish();
        assert_eq!(u32_at(&b, 0), 0x0403_4B50, "primeiro cabeçalho local");
        assert_eq!(u16_at(&b, 8), 0, "guardado, sem compressão");
        assert_eq!(u32_at(&b, 14), crc32(b"ola"));
        assert_eq!(u32_at(&b, 18), 3);
        assert_eq!(&b[30..30 + 15], b"logs/zefiro.log");
        assert_eq!(&b[45..48], b"ola");
        let end = b.len() - 22;
        assert_eq!(u32_at(&b, end), 0x0605_4B50, "fim do diretório central");
        assert_eq!(u16_at(&b, end + 10), 2, "dois arquivos");
        let central = u32_at(&b, end + 16) as usize;
        assert_eq!(u32_at(&b, central), 0x0201_4B50);
        assert_eq!(u32_at(&b, end + 12) as usize, end - central, "tamanho do diretório");
        // O segundo registro central aponta para o segundo cabeçalho local.
        let second = central + 46 + 15;
        let local = u32_at(&b, second + 42) as usize;
        assert_eq!(u32_at(&b, local), 0x0403_4B50);
        assert_eq!(&b[local + 30..local + 41], b"config.toml");
    }

    #[test]
    fn sanitize_hides_the_home_folder_in_every_spelling() {
        let home = r"C:\Users\felip";
        let t = r#"lendo C:\Users\felip\.claude e c:\users\FELIP\AppData; json "C:\\Users\\felip\\x"; C:/Users/felip/y"#;
        let s = sanitize(t, home, "felip");
        assert!(!s.to_lowercase().contains("felip"), "{s}");
        assert!(s.contains(r"%USERPROFILE%\.claude"), "{s}");
        assert!(s.contains("%USERPROFILE%/y"), "{s}");
    }

    #[test]
    fn sanitize_hides_user_name_emails_and_tokens() {
        let t = "usuário felip; mail felipe.cavalari@gbpa.com.br; chave sk-ant-api03-AbCdEfGhIjKlMnOpQrSt; Authorization: Bearer eyJhbGciOi.abc; id 3f9a1c0e7b2d4e6f8a9b0c1d2e3f4a5b6c7d";
        let s = sanitize(t, r"C:\Users\felip", "felip");
        assert!(!s.contains("felip ") && s.contains("<usuario>"), "{s}");
        assert!(!s.contains("gbpa") && s.contains("<email>"), "{s}");
        assert!(!s.contains("AbCdEf") && !s.contains("eyJhbGci") && !s.contains("3f9a1c0e"), "{s}");
        assert!(s.contains("Bearer <token>"), "{s}");
    }

    #[test]
    fn sanitize_keeps_ordinary_log_lines() {
        let line = "2026-10-20T14:03:01Z  INFO zefiro: primeiro quadro da barra ms=38 monitors=2 SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify";
        assert_eq!(sanitize(line, r"C:\Users\felip", "felip"), line);
    }

    #[test]
    fn diagnostic_zip_takes_logs_crashes_config_and_info_scrubbed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("zefiro.2026-10-20.log"), r"lendo C:\Users\felip\.claude").unwrap();
        std::fs::write(dir.path().join("crash-1-2-3.txt"), "panic em felip").unwrap();
        std::fs::write(dir.path().join("outro.bin"), "fora").unwrap();
        let zip = diagnostic_zip(dir.path(), Some("theme = \"cobalto\""), "zefiro 1.0.1", r"C:\Users\felip", "felip", (0, 0x21));
        let text = String::from_utf8_lossy(&zip);
        for name in ["logs/zefiro.2026-10-20.log", "logs/crash-1-2-3.txt", "config.toml", "info.txt"] {
            assert!(text.contains(name), "{name}");
        }
        assert!(!text.contains("outro.bin") && !text.contains("fora"));
        assert!(!text.contains("felip"), "{text}");
        assert!(text.contains("%USERPROFILE%") && text.contains("<usuario>"));
    }

    #[test]
    fn short_user_names_are_not_replaced_everywhere() {
        // "ana" apareceria dentro de "banana" e "análise"; nomes curtos só saem pela pasta.
        assert_eq!(sanitize("banana", r"C:\Users\ana", "an"), "banana");
    }
}
