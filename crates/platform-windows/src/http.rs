//! HTTP bloqueante com o `HttpClient` do Windows (sem dependência nova). Só em threads de trabalho.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use windows::Foundation::Uri;
use windows::Storage::Streams::DataReader;
use windows::Web::Http::Filters::{HttpBaseProtocolFilter, HttpCacheReadBehavior, HttpCacheWriteBehavior};
use windows::Web::Http::{HttpClient, HttpMethod, HttpRequestMessage, HttpResponseMessageSource};
use windows::core::{HSTRING, Result};

pub const USER_AGENT: &str = "Zefiro/0.1 (barra de desktop; https://github.com/Felipe-Cavalari/zefiro)";

/// Sem o cache do Windows: com o filtro padrão, a 2ª consulta em diante devolvia a 1ª resposta do
/// processo (o chip do Claude parava até reiniciar a barra; medido em 2026-10-21).
fn client() -> Result<HttpClient> {
    let filter = HttpBaseProtocolFilter::new()?;
    let cache = filter.CacheControl()?;
    cache.SetReadBehavior(HttpCacheReadBehavior::NoCache)?;
    cache.SetWriteBehavior(HttpCacheWriteBehavior::NoCache)?;
    let client = HttpClient::Create(&filter)?;
    let _ = client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT));
    Ok(client)
}

/// Corpo de uma resposta de sucesso (erro em qualquer outro status ou falha de rede).
pub fn get_string(url: &str) -> Result<String> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    Ok(client()?.GetStringAsync(&uri)?.get()?.to_string())
}

/// Status e se a resposta veio do cache do `HttpClient` (sonda `http_cache_probe`).
pub fn get_source(url: &str) -> Result<(u16, bool)> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let response = client()?.GetAsync(&uri)?.get()?;
    Ok((response.StatusCode()?.0 as u16, response.Source()? == HttpResponseMessageSource::Cache))
}

/// Status e corpo, para distinguir "não existe" (404) de falha de rede (erro).
pub fn get(url: &str) -> Result<(u16, String)> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let response = client()?.GetAsync(&uri)?.get()?;
    let status = response.StatusCode()?.0 as u16;
    let body = response.Content()?.ReadAsStringAsync()?.get()?.to_string();
    Ok((status, body))
}

/// Como `get`, com cabeçalhos próprios (ex.: `Authorization`) e tempo limite: passado `timeout`, a
/// requisição é cancelada e volta erro.
pub fn get_with(url: &str, headers: &[(&str, &str)], timeout: Duration) -> Result<(u16, String)> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let request = HttpRequestMessage::Create(&HttpMethod::Get()?, &uri)?;
    let request_headers = request.Headers()?;
    for (name, value) in headers {
        request_headers.TryAppendWithoutValidation(&HSTRING::from(*name), &HSTRING::from(*value))?;
    }
    let op = client()?.SendRequestAsync(&request)?;
    let (done, wait) = mpsc::channel::<()>();
    let watched = op.clone();
    let watchdog = std::thread::spawn(move || {
        if let Err(RecvTimeoutError::Timeout) = wait.recv_timeout(timeout) {
            let _ = watched.Cancel();
        }
    });
    let result = op.get();
    drop(done);
    let _ = watchdog.join();
    let response = result?;
    let status = response.StatusCode()?.0 as u16;
    let body = response.Content()?.ReadAsStringAsync()?.get()?.to_string();
    Ok((status, body))
}

/// Status e bytes do corpo (o instalador de uma atualização), com tempo limite.
pub fn get_bytes_with(url: &str, timeout: Duration) -> Result<(u16, Vec<u8>)> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    let op = client()?.GetAsync(&uri)?;
    let (done, wait) = mpsc::channel::<()>();
    let watched = op.clone();
    let watchdog = std::thread::spawn(move || {
        if let Err(RecvTimeoutError::Timeout) = wait.recv_timeout(timeout) {
            let _ = watched.Cancel();
        }
    });
    let result = op.get().and_then(|response| {
        let status = response.StatusCode()?.0 as u16;
        let buffer = response.Content()?.ReadAsBufferAsync()?.get()?;
        let mut bytes = vec![0u8; buffer.Length()? as usize];
        DataReader::FromBuffer(&buffer)?.ReadBytes(&mut bytes)?;
        Ok((status, bytes))
    });
    drop(done);
    let _ = watchdog.join();
    result
}
