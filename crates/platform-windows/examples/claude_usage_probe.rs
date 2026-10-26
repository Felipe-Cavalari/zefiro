//! Sonda: as contas do Claude que o Zefiro acha (Windows e WSL rodando) e o uso de cada uma. Imprime
//! a origem, o plano, se há e-mail e o status; nunca o token, o e-mail ou o identificador da conta.
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use zefiro_platform_windows::{claude::ClaudeService, wsl};

fn main() {
    let home = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default());
    let (tx, rx) = mpsc::channel();
    let svc = ClaudeService::start(Arc::new(move || wsl::discover(&home)), Arc::new(move || { let _ = tx.send(()); })).expect("serviço");
    rx.recv_timeout(Duration::from_secs(30)).expect("primeira consulta");
    let snap = svc.latest().expect("snapshot");
    let many = zefiro_integration_usage::origins::many_distros(&snap.accounts);
    for a in &snap.accounts {
        let status = match &a.data.result {
            Some(Ok(u)) => format!("Ok 5h={:?} 7d={:?}", u.five_hour.map(|l| l.percent), u.seven_day.map(|l| l.percent)),
            Some(Err(e)) => format!("{e:?}"),
            None => "sem consulta".into(),
        };
        let origins = zefiro_integration_usage::origins::origins_label(&a.origins, many);
        println!("{origins}: plano={:?} tem e-mail={} {status}", a.data.plan, a.data.email.is_some());
    }
    println!("WSL parado: {}", snap.wsl_stopped);
}
