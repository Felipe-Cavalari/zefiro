//! HTTP bloqueante com o `HttpClient` do Windows (sem dependência nova). Só em threads de trabalho.

use windows::Foundation::Uri;
use windows::Web::Http::HttpClient;
use windows::core::{HSTRING, Result};

pub const USER_AGENT: &str = "Zefiro/0.1 (barra de desktop; https://github.com/Felipe-Cavalari/zefiro)";

pub fn get_string(url: &str) -> Result<String> {
    let client = HttpClient::new()?;
    let _ = client.DefaultRequestHeaders()?.UserAgent()?.TryParseAdd(&HSTRING::from(USER_AGENT));
    let uri = Uri::CreateUri(&HSTRING::from(url))?;
    Ok(client.GetStringAsync(&uri)?.get()?.to_string())
}
