//! Sonda do chat: sobe o `claude` pela origem pedida, faz duas perguntas (a segunda depende da
//! primeira), para uma resposta longa no meio e retoma a sessão num processo novo. Imprime só
//! eventos e tamanhos, nunca o texto. Sai com 1 se algo falhar.
//!
//! `ZEFIRO_CHAT_ORIGIN=auto|windows|wsl|<distro>` (padrão `auto`), `ZEFIRO_CHAT_MODEL=<modelo>`.

use std::time::{Duration, Instant};
use zefiro_integration_chat::conversation::Conversation;
use zefiro_integration_chat::origin::parse_choice;
use zefiro_integration_chat::protocol::{Event, classify, user_line};
use zefiro_platform_windows::chat::{ChatService, Signal, Start, new_session_id};

fn wait_turn(service: &ChatService, c: &mut Conversation, limit: Duration, stop_after: Option<Duration>) -> Result<Duration, String> {
    let started = Instant::now();
    let mut first = None;
    let mut stopped = false;
    while started.elapsed() < limit {
        for s in service.take() {
            match s {
                Signal::Ready { origin } => println!("  pronto em {origin}"),
                Signal::Event(Event::Finished { error: Some(e), .. }) => {
                    c.fail(&classify(&e));
                    return if stopped { Ok(started.elapsed()) } else { Err(format!("erro no fim do turno: {e}")) };
                }
                Signal::Event(e) => {
                    if matches!(e, Event::Delta(_)) && first.is_none() {
                        first = Some(started.elapsed());
                        println!("  primeiro texto em {:.1} s", started.elapsed().as_secs_f32());
                    }
                    c.apply(&e);
                    if !c.busy() {
                        return Ok(started.elapsed());
                    }
                }
                Signal::Exited { stderr } => return Err(format!("processo saiu: {stderr}")),
                Signal::Failed(text) => return Err(text),
            }
        }
        if let (Some(after), Some(f)) = (stop_after, first)
            && !stopped
            && started.elapsed() > f + after
        {
            println!("  parando a resposta");
            c.stop_requested();
            service.interrupt();
            stopped = true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err("tempo esgotado".into())
}

fn ask(service: &ChatService, c: &mut Conversation, text: &str, stop_after: Option<Duration>) -> Result<(), String> {
    c.ask(text);
    service.send(user_line(text));
    let took = wait_turn(service, c, Duration::from_secs(120), stop_after)?;
    let reply = c.messages.last().unwrap();
    println!("  turno em {:.1} s: {:?}, {} caracteres", took.as_secs_f32(), reply.state, reply.text.chars().count());
    Ok(())
}

fn run() -> Result<(), String> {
    let choice = parse_choice(&std::env::var("ZEFIRO_CHAT_ORIGIN").unwrap_or_default());
    let model = std::env::var("ZEFIRO_CHAT_MODEL").unwrap_or_default();
    let dir = std::env::temp_dir().join("zefiro-chat-probe");
    let service = ChatService::new(0).map_err(|e| e.to_string())?;
    let mut c = Conversation::new(new_session_id());
    let start = |c: &Conversation| Start { choice: choice.clone(), model: model.clone(), session: c.session.clone(), resume: c.created, windows_dir: dir.clone() };
    println!("1. pergunta com o processo frio");
    service.start(start(&c));
    ask(&service, &mut c, "Guarde a palavra-chave: girassol. Responda só: ok", None)?;
    println!("2. resposta longa parada no meio");
    ask(&service, &mut c, "Conte de 1 a 300 por extenso, um número por linha.", Some(Duration::from_millis(1500)))?;
    println!("3. processo novo retomando a sessão");
    service.stop();
    service.start(start(&c));
    ask(&service, &mut c, "Qual era a palavra-chave? Responda só a palavra.", None)?;
    let remembered = c.last_reply().is_some_and(|r| r.to_lowercase().contains("girassol"));
    println!("   lembrou do contexto: {remembered}");
    service.stop();
    std::thread::sleep(Duration::from_secs(2));
    if remembered { Ok(()) } else { Err("não lembrou da primeira pergunta".into()) }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("falhou: {e}");
        std::process::exit(1);
    }
}
