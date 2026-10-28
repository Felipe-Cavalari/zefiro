use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use zefiro_platform_windows::spotify_auth::{ListenResult, listen_on};
use zefiro_platform_windows::{http, secret};

fn winrt() {
    // SAFETY: inicializa o WinRT na thread do teste (HttpClient); nunca desfeito, o processo acaba.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

/// Servidor de uma resposta só: devolve `response` e manda o pedido recebido por `tx`.
fn one_shot(response: &'static str) -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // Lê o cabeçalho e o corpo (até o tempo acabar).
        while let Ok(n) = s.read(&mut chunk) {
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf);
            if let Some(end) = text.find("\r\n\r\n") {
                let len = text[..end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
        }
        s.write_all(response.as_bytes()).unwrap();
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    (port, rx)
}

#[test]
fn send_puts_method_headers_and_body() {
    winrt();
    let (port, rx) = one_shot("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    let url = format!("http://127.0.0.1:{port}/v1/me/player/play");
    let (status, body, retry) = http::send("PUT", &url, &[("Authorization", "Bearer X")], Some(("application/json", "{\"a\":1}")), Duration::from_secs(5)).unwrap();
    assert_eq!((status, body.as_str(), retry), (204, "", None));
    let request = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.starts_with("PUT /v1/me/player/play HTTP/1.1"), "{request}");
    assert!(request.to_ascii_lowercase().contains("content-type: application/json"), "{request}");
    assert!(request.contains("Authorization: Bearer X"), "{request}");
    assert!(request.ends_with("{\"a\":1}"), "{request}");
}

#[test]
fn send_reads_retry_after() {
    winrt();
    let (port, _rx) = one_shot("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 7\r\nContent-Length: 2\r\n\r\n{}");
    let (status, body, retry) = http::send("GET", &format!("http://127.0.0.1:{port}/x"), &[], None, Duration::from_secs(5)).unwrap();
    assert_eq!((status, body.as_str(), retry), (429, "{}", Some(7)));
}

#[test]
fn dpapi_round_trips_and_is_not_plain() {
    let secret_text = b"refresh-token-de-teste";
    let sealed = secret::protect(secret_text).unwrap();
    assert!(!sealed.windows(secret_text.len()).any(|w| w == secret_text), "cifrado, não em claro");
    assert_eq!(secret::unprotect(&sealed).unwrap(), secret_text);
    assert!(secret::unprotect(b"lixo").is_err());
}

#[test]
fn random_is_not_constant() {
    let a: [u8; 32] = secret::random();
    let b: [u8; 32] = secret::random();
    assert_ne!(a, b);
    assert_ne!(a, [0u8; 32]);
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn listener_returns_code_and_serves_done_page() {
    let port = free_port();
    let (tx, rx) = mpsc::channel();
    listen_on(port, "s".into(), Duration::from_secs(5), move |r| tx.send(r).unwrap()).unwrap();
    // Um pedido que não é o callback (o favicon) é ignorado; o state errado também.
    for path in ["/favicon.ico", "/callback?code=no&state=outro", "/callback?code=xyz&state=s"] {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).unwrap();
        let mut page = String::new();
        let _ = s.read_to_string(&mut page);
        if path.contains("xyz") {
            assert!(page.contains("Pronto, pode voltar ao Zefiro"), "{page}");
        }
    }
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), ListenResult::Code("xyz".into()));
}

#[test]
fn listener_reports_denial() {
    let port = free_port();
    let (tx, rx) = mpsc::channel();
    listen_on(port, "s".into(), Duration::from_secs(5), move |r| tx.send(r).unwrap()).unwrap();
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"GET /callback?error=access_denied&state=s HTTP/1.1\r\n\r\n").unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), ListenResult::Denied);
}

#[test]
fn listener_reports_port_busy() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let err = listen_on(port, "s".into(), Duration::from_secs(1), |_| {}).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
}

#[test]
fn listener_times_out() {
    let port = free_port();
    let (tx, rx) = mpsc::channel();
    listen_on(port, "s".into(), Duration::from_millis(200), move |r| tx.send(r).unwrap()).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), ListenResult::TimedOut);
}

#[test]
fn send_without_body_still_says_content_length_zero() {
    winrt();
    let (port, rx) = one_shot("HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
    http::send("POST", &format!("http://127.0.0.1:{port}/v1/me/player/next"), &[], None, Duration::from_secs(5)).unwrap();
    let request = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.to_ascii_lowercase().contains("content-length: 0"), "o Spotify responde 411 sem isso: {request}");
}
