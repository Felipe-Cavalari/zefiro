//! HTTP bloqueante com o `HttpClient` do Windows (sem dependência nova). Só em threads de trabalho.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use windows::Foundation::Uri;
use windows::Web::Http::{HttpClient, HttpMethod, HttpRequestMessage};
use windows::core::{HSTRING, Result};

pub const USER_AGENT: &str = "Zefiro/0.1 (barra de desktop; https://github.com/Felipe-Cavalari/zefiro)";

fn client() -> Result<HttpClient> {
    let client = HttpClient::new()?;
    let _ = client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT));
    Ok(client)
}

/// Corpo de uma resposta de sucesso (erro em qualquer outro status ou falha de rede).
pub fn get_string(url: &str) -> Result<String> {
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    Ok(client()?.GetStringAsync(&uri)?.get()?.to_string())
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
