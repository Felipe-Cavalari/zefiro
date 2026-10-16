//! Saída de som por fonte de mídia (porte de `island/zefiro_audio_route.cpp`, ADR-0006): como achar
//! os processos de um app a partir do AUMID do SMTC e como normalizar o id do dispositivo.

/// Como casar um processo com a fonte de mídia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMatcher {
    /// Apps da Store: a família do pacote (o que vem antes do `!`), em minúsculas.
    pub package_family: Option<String>,
    /// Os outros: nomes de executável aceitos (minúsculos, sem `.exe`).
    pub keys: Vec<String>,
}

fn strip_exe(s: &str) -> &str {
    if s.len() > 4 && s.to_ascii_lowercase().ends_with(".exe") { &s[..s.len() - 4] } else { s }
}

impl SourceMatcher {
    pub fn new(aumid: &str, source_name: &str) -> SourceMatcher {
        if let Some((family, _)) = aumid.split_once('!') {
            return SourceMatcher { package_family: Some(family.to_lowercase()), keys: Vec::new() };
        }
        let mut keys: Vec<String> = Vec::new();
        let mut add = |k: String| {
            if !k.is_empty() && !keys.contains(&k) {
                keys.push(k);
            }
        };
        // "Spotify.exe", "C:\...\app.exe", "Brave", "MSEdge", "Brave.XXXXXXXX" (perfil).
        let base = aumid.rsplit(['\\', '/']).next().unwrap_or(aumid).to_lowercase();
        let full = strip_exe(&base).to_string();
        add(full.clone());
        add(full.split('.').next().unwrap_or(&full).to_string());
        // AUMID opaco (o Firefox usa um hash): ainda casa pelo nome mostrado.
        let name: String = source_name.to_lowercase().chars().filter(|c| *c != ' ').collect();
        if name != "media" {
            add(name);
        }
        SourceMatcher { package_family: None, keys }
    }

    /// Casa pelo nome do executável (só para apps fora da Store).
    pub fn matches_exe(&self, exe_name: &str) -> bool {
        self.package_family.is_none() && self.keys.iter().any(|k| *k == strip_exe(&exe_name.to_lowercase()))
    }
}

const MMDEVAPI_PREFIX: &str = r"\\?\SWD#MMDEVAPI#";
const RENDER_SUFFIX: &str = "#{e6327cad-dcec-4949-ae8a-991e976a79d2}";

/// A política guarda o id no formato de interface; o `IMMDevice` usa o id curto.
pub fn device_interface_id(device_id: &str) -> String {
    format!("{MMDEVAPI_PREFIX}{device_id}{RENDER_SUFFIX}")
}

/// Inverso de `device_interface_id` (sem diferenciar maiúsculas).
pub fn device_id_from_interface(interface: &str) -> String {
    let mut id = interface;
    if id.len() >= MMDEVAPI_PREFIX.len() && id[..MMDEVAPI_PREFIX.len()].eq_ignore_ascii_case(MMDEVAPI_PREFIX) {
        id = &id[MMDEVAPI_PREFIX.len()..];
    }
    if id.len() >= RENDER_SUFFIX.len() && id[id.len() - RENDER_SUFFIX.len()..].eq_ignore_ascii_case(RENDER_SUFFIX) {
        id = &id[..id.len() - RENDER_SUFFIX.len()];
    }
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_apps_match_by_package_family() {
        let m = SourceMatcher::new("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", "Spotify");
        assert_eq!(m.package_family.as_deref(), Some("spotifyab.spotifymusic_zpdnekdrzrea0"));
        assert!(!m.matches_exe("Spotify.exe"), "Store casa pela família, não pelo exe");
    }

    #[test]
    fn desktop_apps_match_by_exe_name_and_profile_prefix() {
        let brave = SourceMatcher::new("Brave.ABCDEF123", "Brave");
        assert!(brave.matches_exe("brave.exe"));
        assert!(SourceMatcher::new("Spotify.exe", "Spotify").matches_exe("Spotify.EXE"));
        assert!(SourceMatcher::new(r"C:\Apps\foobar2000.exe", "foobar2000").matches_exe("foobar2000.exe"));
        let firefox = SourceMatcher::new("308046B0AF4A39CB", "Firefox");
        assert!(firefox.matches_exe("firefox.exe"), "AUMID opaco casa pelo nome mostrado");
        assert!(!brave.matches_exe("chrome.exe"));
    }

    #[test]
    fn device_ids_round_trip_through_the_interface_format() {
        let id = "{0.0.0.00000000}.{6d2b8b9c-1111-2222-3333-444455556666}";
        let interface = device_interface_id(id);
        assert!(interface.starts_with(r"\\?\SWD#MMDEVAPI#{0.0.0"));
        assert_eq!(device_id_from_interface(&interface.to_uppercase()).to_lowercase(), id.to_lowercase());
        assert_eq!(device_id_from_interface(""), "");
    }
}
