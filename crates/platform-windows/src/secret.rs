//! Segredos do usuário: cifrar com o DPAPI (escopo do usuário, sem interface) e bytes aleatórios do
//! gerador do sistema. É assim que o token de renovação do Spotify vai para o disco (ADR-0024).

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData};
use windows::core::{PCWSTR, Result};

/// Copia o blob devolvido pelo DPAPI e libera a memória dele.
fn take(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    // SAFETY: o DPAPI devolveu `cbData` bytes válidos em `pbData`, alocados com LocalAlloc.
    let out = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    // SAFETY: par do LocalAlloc feito pelo DPAPI; o ponteiro não é usado depois.
    unsafe { LocalFree(Some(HLOCAL(blob.pbData.cast()))) };
    out
}

pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr().cast_mut() };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` aponta para `data`, vivo durante a chamada; o DPAPI não escreve nele.
    unsafe { CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output) }?;
    Ok(take(output))
}

pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr().cast_mut() };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: como em `protect`.
    unsafe { CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output) }?;
    Ok(take(output))
}

/// `N` bytes do gerador do sistema.
pub fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    // SAFETY: `out` é um buffer válido de N bytes; sem handle de algoritmo, vale o padrão do sistema.
    let status = unsafe { BCryptGenRandom(None, &mut out, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    debug_assert!(status.is_ok(), "BCryptGenRandom falhou: {status:?}");
    out
}
