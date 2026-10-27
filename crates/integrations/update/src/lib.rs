//! Atualização pelo GitHub Releases, portável (ADR-0014): a versão, a leitura da última release, a
//! escolha do instalador, a origem confiável do download e a conferência do SHA-256. A rede e o
//! instalador ficam na `platform-windows`.

use serde::Deserialize;
use std::fmt;

/// Última release publicada (a API ignora drafts e pré-releases).
pub const LATEST_URL: &str = "https://api.github.com/repos/Felipe-Cavalari/zefiro/releases/latest";
/// Só downloads de releases deste repositório viram atualização.
const DOWNLOAD_PREFIX: &str = "https://github.com/Felipe-Cavalari/zefiro/releases/download/";

/// Versão `maior.menor.correção`; sufixos (`-beta`) não são aceitos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let t = text.trim();
        let t = t.strip_prefix('v').unwrap_or(t);
        let mut parts = t.split('.').map(|p| p.parse::<u32>().ok());
        let major = parts.next()??;
        let minor = parts.next().unwrap_or(Some(0))?;
        let patch = parts.next().unwrap_or(Some(0))?;
        parts.next().is_none().then_some(Version(major, minor, patch))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// O instalador anexado à release.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset { pub name: String, pub url: String, pub size: u64, pub sha256: Option<String> }

#[derive(Debug, Clone, PartialEq)]
pub struct Release { pub version: Version, pub tag: String, pub page: String, pub asset: Asset }

#[derive(Deserialize)]
struct RawAsset { name: String, size: u64, browser_download_url: String, #[serde(default)] digest: Option<String> }

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

/// Lê a resposta de `LATEST_URL`. Erro (em pt-BR, para o log) se não for uma release estável deste
/// repositório com um `Zefiro-*-setup.exe`.
pub fn parse_latest(json: &str) -> Result<Release, String> {
    let raw: RawRelease = serde_json::from_str(json).map_err(|e| format!("resposta inesperada do GitHub: {e}"))?;
    if raw.draft || raw.prerelease {
        return Err("a última release é rascunho ou pré-release".into());
    }
    let version = Version::parse(&raw.tag_name).ok_or_else(|| format!("tag sem versão: {}", raw.tag_name))?;
    let asset = raw
        .assets
        .into_iter()
        .find(|a| a.name.starts_with("Zefiro-") && a.name.ends_with("-setup.exe"))
        .ok_or("a release não tem o Zefiro-*-setup.exe")?;
    if !trusted_url(&asset.browser_download_url) {
        return Err(format!("download fora do repositório: {}", asset.browser_download_url));
    }
    let sha256 = asset.digest.and_then(|d| d.strip_prefix("sha256:").map(str::to_ascii_lowercase));
    Ok(Release { version, tag: raw.tag_name, page: raw.html_url, asset: Asset { name: asset.name, url: asset.browser_download_url, size: asset.size, sha256 } })
}

/// A release, se for mais nova que `current` (a versão deste exe).
pub fn update_for(current: &str, json: &str) -> Option<Release> {
    let current = Version::parse(current)?;
    parse_latest(json).ok().filter(|r| r.version > current)
}

pub fn trusted_url(url: &str) -> bool {
    url.starts_with(DOWNLOAD_PREFIX) && !url.contains("..")
}

/// Confere o arquivo baixado: o tamanho e, quando o GitHub publicou, o SHA-256.
pub fn verify(bytes: &[u8], asset: &Asset) -> Result<(), String> {
    if bytes.len() as u64 != asset.size {
        return Err(format!("tamanho {} em vez de {}", bytes.len(), asset.size));
    }
    match &asset.sha256 {
        Some(expected) if &sha256_hex(bytes) != expected => Err("o SHA-256 não confere".into()),
        _ => Ok(()),
    }
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 (FIPS 180-4) em hexadecimal minúsculo.
pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 (FIPS 180-4) em bytes. Próprio, para não puxar dependência por ~40 linhas.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, x) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *chunk = x.to_be_bytes();
    }
    out
}
