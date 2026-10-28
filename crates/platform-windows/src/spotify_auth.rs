//! O lado Windows do login no Spotify: o listener numa porta livre de `127.0.0.1` que recebe o
//! código do navegador e o transporte HTTP da sessão (ADR-0024).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use zefiro_integration_spotify::auth::{Callback, DONE_PAGE, parse_callback};
use zefiro_integration_spotify::session::{NetworkError, Reply, Transport};

const DENIED_PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Zefiro</title><body style=\"font-family:Segoe UI,sans-serif;background:#08080A;color:#fff;display:flex;align-items:center;justify-content:center;height:90vh\"><p>Login cancelado. Pode fechar esta aba.</p></body></html>";
const NOT_FOUND: &str = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenResult { Code(String), Denied, TimedOut }

/// 1ª linha do pedido HTTP (até 8 KB ou 2 s).
fn request_head(stream: &mut TcpStream) -> String {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while buf.len() < 8192 && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&buf).lines().next().unwrap_or_default().to_string()
}

/// Espera o callback numa porta livre de `127.0.0.1` (o Windows escolhe), numa thread; devolve a
/// porta, e `done` recebe o resultado uma vez.
pub fn listen(state: String, timeout: Duration, done: impl FnOnce(ListenResult) + Send + 'static) -> std::io::Result<u16> {
    listen_on(0, state, timeout, done)
}

/// Como `listen`, numa porta escolhida (testes). A porta ocupada volta como erro aqui mesmo.
pub fn listen_on(port: u16, state: String, timeout: Duration, done: impl FnOnce(ListenResult) + Send + 'static) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    std::thread::Builder::new().name("zefiro-spotify-login".into()).spawn(move || {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let head = request_head(&mut stream);
                    match parse_callback(&head, &state) {
                        Callback::Code(code) => {
                            let _ = stream.write_all(DONE_PAGE.as_bytes());
                            done(ListenResult::Code(code));
                            return;
                        }
                        Callback::Denied(_) => {
                            let _ = stream.write_all(DENIED_PAGE.as_bytes());
                            done(ListenResult::Denied);
                            return;
                        }
                        // O favicon e pedidos com outro `state` não encerram a espera.
                        Callback::WrongState | Callback::NotCallback => {
                            let _ = stream.write_all(NOT_FOUND.as_bytes());
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => {
                    tracing::warn!("spotify: listener do login falhou: {e}");
                    break;
                }
            }
        }
        done(ListenResult::TimedOut);
    })?;
    Ok(port)
}

/// A sessão do Spotify sobre o `HttpClient` do Windows. Só em threads de trabalho com WinRT.
pub struct HttpTransport;

impl Transport for HttpTransport {
    fn send(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &str)>) -> Reply {
        crate::http::send(method, url, headers, body, HTTP_TIMEOUT).map_err(|e| {
            tracing::debug!("spotify: falha de rede: {}", e.message());
            NetworkError
        })
    }
}
