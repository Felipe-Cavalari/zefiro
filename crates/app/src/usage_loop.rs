//! Uso do Claude e do Codex no laço de eventos: serviços, chips e painéis.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::usage::{UsageBlock, UsageRow, usage_panel};
use zefiro_integration_clock::windows::utc_offset_seconds;
use zefiro_integration_usage::claude::{ClaudeSnapshot, Limit, UsageError};
use zefiro_integration_usage::codex::{CodexService, CodexSnapshot, Window, effective_percent, window_name};
use zefiro_integration_usage::origins::{OriginKind, many_distros, origins_label};
use zefiro_integration_usage::format::{format_absolute, format_relative, format_short, parse_iso_utc};
use zefiro_platform_windows::claude::ClaudeService;
use zefiro_platform_windows::{window, wsl};
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_CLAUDE, USER_CODEX};

pub const CODEX_GLYPH: &str = "\u{E943}";
pub const CLAUDE_GLYPH: &str = "\u{E945}";

#[derive(Default)]
pub struct UsageRuntime {
    codex: Option<CodexService>,
    claude: Option<ClaudeService>,
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A pasta pessoal do Windows. `ZEFIRO_USAGE_WINDOWS_HOME` troca por outra só para testes (simular
/// um tester que só usa o WSL).
fn windows_home() -> PathBuf {
    std::env::var_os("ZEFIRO_USAGE_WINDOWS_HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

const WSL_STOPPED: &str = "WSL desligado.\nAbra o WSL para ver o uso de lá.";
const STOPPED_SUFFIX: &str = " · WSL desligado";


/// "reinicia em 2 h 14 min" para janelas de até um dia; "reinicia qui 22 out, 11:08" para as
/// maiores; "reiniciada" se já passou.
fn reset_detail(resets_at: i64, window_minutes: u32, now: i64, offset: i32) -> String {
    if resets_at <= now {
        "reiniciada".into()
    } else if window_minutes <= 1440 {
        format!("reinicia em {}", format_relative(resets_at - now))
    } else {
        format!("reinicia {}", format_absolute(resets_at, offset))
    }
}

fn claude_row(label: String, limit: &Limit, window_minutes: u32, now: i64, offset: i32) -> UsageRow {
    let detail = limit.resets_at.map(|t| reset_detail(t, window_minutes, now, offset)).unwrap_or_default();
    UsageRow { label, percent: Some(limit.percent), detail }
}

/// `origin`: "Windows" ou "WSL", onde abrir o Claude Code para renovar o login.
fn claude_message(e: UsageError, origin: &str) -> String {
    match e {
        UsageError::Expired => format!("Login expirado no {origin}.\nAbra o Claude Code no {origin} para renovar."),
        UsageError::Missing => "Faça login no Claude Code".into(),
        UsageError::Network => "Sem conexão.\nTento de novo em 2 min.".into(),
        UsageError::UnknownFormat => "Formato de resposta desconhecido".into(),
        UsageError::Http(status) => format!("O serviço respondeu {status}.\nTento de novo em 2 min."),
    }
}

fn updated_ago(at: std::time::Instant) -> String {
    match at.elapsed().as_secs() / 60 {
        0 => "Atualizado agora".into(),
        m => format!("Atualizado há {m} min"),
    }
}

fn codex_row(w: &Window, now: i64, offset: i32) -> UsageRow {
    UsageRow { label: window_name(w.window_minutes), percent: Some(effective_percent(w, now)), detail: reset_detail(w.resets_at, w.window_minutes, now, offset) }
}

fn block(title: String, subtitle: String, rows: Vec<UsageRow>, message: Option<String>) -> UsageBlock {
    UsageBlock { title, subtitle, rows, message }
}

/// Um bloco por conta do Claude; sem contas, um bloco com o motivo.
fn claude_blocks(s: &ClaudeSnapshot, now: i64, offset: i32) -> Vec<UsageBlock> {
    if s.accounts.is_empty() {
        let message = if s.wsl_stopped { WSL_STOPPED } else { "Faça login no Claude Code" };
        return vec![block("Claude".into(), String::new(), Vec::new(), Some(message.into()))];
    }
    let many = many_distros(&s.accounts);
    s.accounts
        .iter()
        .map(|a| {
            let title = a.data.plan.as_deref().map_or("Claude".to_string(), |p| format!("Claude · {p}"));
            let origins = origins_label(&a.origins, many);
            let mut subtitle = a.data.email.as_deref().map_or(origins.clone(), |e| format!("{e} · {origins}"));
            if a.stopped {
                subtitle.push_str(STOPPED_SUFFIX);
            }
            let renew_at = if a.origins.contains(&OriginKind::Windows) { "Windows" } else { "WSL" };
            // Conta lembrada de uma distro parada: o último valor bom, sem mensagem de erro.
            let usage = match &a.data.result {
                Some(Ok(u)) => Ok(Some(u)),
                Some(Err(_)) if a.stopped && a.data.last_ok.is_some() => Ok(a.data.last_ok.as_ref()),
                Some(Err(e)) => Err(claude_message(*e, renew_at)),
                None => Ok(None),
            };
            match usage {
                Err(message) => block(title, subtitle, Vec::new(), Some(message)),
                Ok(None) => block(title, subtitle, Vec::new(), Some("Carregando…".into())),
                Ok(Some(u)) => {
                    let mut rows = Vec::new();
                    if let Some(l) = &u.five_hour {
                        rows.push(claude_row("5 horas".into(), l, 300, now, offset));
                    }
                    if let Some(l) = &u.seven_day {
                        rows.push(claude_row("7 dias".into(), l, 10080, now, offset));
                    }
                    for (name, l) in &u.extra {
                        rows.push(claude_row(format!("{name} (7 dias)"), l, 10080, now, offset));
                    }
                    block(title, subtitle, rows, None)
                }
            }
        })
        .collect()
}

/// Um bloco por conta do Codex; com mais de uma conta, o título diz a origem.
fn codex_blocks(s: &CodexSnapshot, now: i64, offset: i32) -> Vec<UsageBlock> {
    if s.accounts.is_empty() {
        let message = if s.wsl_stopped { WSL_STOPPED } else { "Nenhum uso do Codex encontrado" };
        return vec![block("Codex".into(), String::new(), Vec::new(), Some(message.into()))];
    }
    let many = many_distros(&s.accounts);
    s.accounts
        .iter()
        .map(|a| {
            let u = &a.data.usage;
            let mut title = u.plan.as_deref().map_or("Codex".to_string(), |p| format!("Codex · {}", capitalized(p)));
            if s.accounts.len() > 1 {
                title = format!("{title} · {}", origins_label(&a.origins, many));
            }
            let mut subtitle = u.event_at.as_deref().and_then(parse_iso_utc).map(|t| format!("Último uso do Codex: {}", format_short(t, offset))).unwrap_or_default();
            if a.stopped {
                subtitle = if subtitle.is_empty() { "WSL desligado".into() } else { format!("{subtitle}{STOPPED_SUFFIX}") };
            }
            let rows = [u.primary, u.secondary].iter().flatten().map(|w| codex_row(w, now, offset)).collect();
            block(title, subtitle, rows, None)
        })
        .collect()
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

impl Loop {
    /// Liga ou desliga o Claude e o Codex conforme os chips estão na barra: sem o chip, o token do
    /// Claude não é lido nem enviado, e a pasta do Codex não é varrida.
    pub fn usage_services(&mut self) {
        let (claude, codex) = (self.ctx.config.bar.uses("claude"), self.ctx.config.bar.uses("codex"));
        match (claude, self.usage.claude.is_some()) {
            (true, false) => {
                tracing::info!("uso do Claude: serviço ligado");
                let home = windows_home();
                self.usage.claude = ClaudeService::start(Arc::new(move || wsl::discover(&home)), Arc::new(|| window::post_user(USER_CLAUDE)))
                    .map_err(|e| tracing::warn!("uso do Claude indisponível: {e}"))
                    .ok();
            }
            (false, true) => {
                tracing::info!("uso do Claude: serviço desligado (chip fora da barra)");
                self.usage.claude = None;
                self.ctx.usage.set_claude(None, now_unix());
            }
            _ => {}
        }
        match (codex, self.usage.codex.is_some()) {
            (true, false) => {
                tracing::info!("uso do Codex: serviço ligado");
                let home = windows_home();
                self.usage.codex = CodexService::start(Arc::new(move || wsl::discover(&home)), Arc::new(|| window::post_user(USER_CODEX)))
                    .map_err(|e| tracing::warn!("uso do Codex indisponível: {e}"))
                    .ok();
            }
            (false, true) => {
                tracing::info!("uso do Codex: serviço desligado (chip fora da barra)");
                self.usage.codex = None;
                self.ctx.usage.set_codex(None, now_unix());
            }
            _ => {}
        }
    }

    fn usage_panel_open(&self, kind: PanelKind) -> bool {
        self.controller.open().is_some_and(|o| o.kind == kind)
    }

    /// Chega quando o uso muda ou uma janela reinicia: a barra só é refeita se o número do chip mudou.
    pub fn usage_codex_changed(&mut self) -> Result<()> {
        let latest = self.usage.codex.as_ref().and_then(CodexService::latest);
        if self.ctx.usage.set_codex(latest, now_unix()) {
            self.apply_bars(true)?;
        }
        if self.usage_panel_open(PanelKind::Codex) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Chega a cada consulta (a cada 2 min): a barra só é refeita se o número do chip mudou; o
    /// painel aberto é redesenhado sempre (rodapé "Atualizado há").
    pub fn usage_claude_changed(&mut self) -> Result<()> {
        let latest = self.usage.claude.as_ref().and_then(ClaudeService::latest);
        if self.ctx.usage.set_claude(latest, now_unix()) {
            self.apply_bars(true)?;
        }
        if self.usage_panel_open(PanelKind::Claude) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Ao abrir o painel: confere o dado agora.
    pub fn usage_prepare(&self, kind: PanelKind) {
        if kind == PanelKind::Codex
            && let Some(s) = &self.usage.codex
        {
            s.refresh();
        }
        if kind == PanelKind::Claude
            && let Some(s) = &self.usage.claude
        {
            s.refresh();
        }
    }

    pub fn usage_panel_list(&self, kind: PanelKind, scale: f32) -> DisplayList {
        let (tokens, text) = (&self.ctx.theme.tokens, &self.ctx.text);
        let (now, offset) = (now_unix(), utc_offset_seconds());
        if kind == PanelKind::Claude {
            return match &self.ctx.usage.claude {
                None => usage_panel(&[block("Claude".into(), String::new(), Vec::new(), Some("Carregando…".into()))], "", scale, tokens, text),
                Some(s) => usage_panel(&claude_blocks(s, now, offset), &s.at.map(updated_ago).unwrap_or_default(), scale, tokens, text),
            };
        }
        let blocks = codex_blocks(self.ctx.usage.codex.as_ref().unwrap_or(&CodexSnapshot::default()), now, offset);
        usage_panel(&blocks, "", scale, tokens, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use zefiro_integration_usage::claude::{ClaudeEntry, ClaudeSnapshot, ClaudeUsage};
    use zefiro_integration_usage::codex::{CodexEntry, CodexSnapshot, CodexUsage};
    use zefiro_integration_usage::origins::Account;

    const NOW: i64 = 1_792_000_000;

    fn claude(origins: Vec<OriginKind>, result: std::result::Result<ClaudeUsage, UsageError>, email: Option<&str>, stopped: bool) -> Account<ClaudeEntry> {
        let data = ClaudeEntry { email: email.map(String::from), plan: Some("Pro".into()), credentials: Vec::new(), result: Some(result), last_ok: None };
        Account { key: format!("{origins:?}"), origins, stopped, data }
    }

    fn usage(five: f32, seven: f32) -> ClaudeUsage {
        ClaudeUsage { five_hour: Some(Limit { percent: five, resets_at: Some(NOW + 3600) }), seven_day: Some(Limit { percent: seven, resets_at: Some(NOW + 86_400) }), extra: Vec::new() }
    }

    fn claude_snapshot(accounts: Vec<Account<ClaudeEntry>>, wsl_stopped: bool) -> ClaudeSnapshot {
        ClaudeSnapshot { accounts, wsl_stopped, at: None }
    }

    fn codex(origins: Vec<OriginKind>, stopped: bool) -> Account<CodexEntry> {
        let window = Window { used_percent: 10.0, window_minutes: 300, resets_at: NOW + 60 };
        let u = CodexUsage { primary: Some(window), secondary: None, plan: Some("plus".into()), event_at: Some("2026-10-21T10:00:00Z".into()) };
        Account { key: format!("{origins:?}"), origins, stopped, data: CodexEntry { modified: SystemTime::UNIX_EPOCH, usage: u } }
    }

    fn wsl() -> OriginKind {
        OriginKind::Wsl("Ubuntu-24.04".into())
    }

    #[test]
    fn claude_single_account_block() {
        let s = claude_snapshot(vec![claude(vec![OriginKind::Windows], Ok(usage(9.0, 18.0)), Some("a@b.c"), false)], false);
        let blocks = claude_blocks(&s, NOW, 0);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].title, "Claude · Pro");
        assert_eq!(blocks[0].subtitle, "a@b.c · Windows");
        assert_eq!(blocks[0].rows.len(), 2);
        assert_eq!(blocks[0].message, None);
    }

    #[test]
    fn claude_expired_wsl_message() {
        let s = claude_snapshot(vec![claude(vec![wsl()], Err(UsageError::Expired), None, false)], false);
        let blocks = claude_blocks(&s, NOW, 0);
        assert_eq!(blocks[0].message.as_deref(), Some("Login expirado no WSL.\nAbra o Claude Code no WSL para renovar."));
        assert_eq!(blocks[0].subtitle, "WSL");
    }

    #[test]
    fn claude_no_accounts_with_wsl_stopped() {
        let stopped = claude_blocks(&claude_snapshot(Vec::new(), true), NOW, 0);
        assert_eq!(stopped[0].title, "Claude");
        assert_eq!(stopped[0].message.as_deref(), Some("WSL desligado.\nAbra o WSL para ver o uso de lá."));
        let none = claude_blocks(&claude_snapshot(Vec::new(), false), NOW, 0);
        assert_eq!(none[0].message.as_deref(), Some("Faça login no Claude Code"));
    }

    #[test]
    fn codex_two_accounts_titles() {
        let s = CodexSnapshot { accounts: vec![codex(vec![OriginKind::Windows], false), codex(vec![wsl()], false)], wsl_stopped: false };
        let titles: Vec<String> = codex_blocks(&s, NOW, 0).into_iter().map(|b| b.title).collect();
        assert_eq!(titles, ["Codex · Plus · Windows", "Codex · Plus · WSL"]);
        let one = CodexSnapshot { accounts: vec![codex(vec![OriginKind::Windows, wsl()], false)], wsl_stopped: false };
        assert_eq!(codex_blocks(&one, NOW, 0)[0].title, "Codex · Plus");
    }

    #[test]
    fn codex_no_accounts_messages() {
        let stopped = codex_blocks(&CodexSnapshot { accounts: Vec::new(), wsl_stopped: true }, NOW, 0);
        assert_eq!(stopped[0].message.as_deref(), Some("WSL desligado.\nAbra o WSL para ver o uso de lá."));
        let none = codex_blocks(&CodexSnapshot::default(), NOW, 0);
        assert_eq!(none[0].message.as_deref(), Some("Nenhum uso do Codex encontrado"));
    }

    #[test]
    fn stopped_account_without_good_value_shows_its_error() {
        // O caso do usuário: token do WSL vencido e a distro para; nunca houve valor bom.
        let s = claude_snapshot(vec![claude(vec![wsl()], Err(UsageError::Expired), Some("a@b.c"), true)], true);
        let b = &claude_blocks(&s, NOW, 0)[0];
        assert_eq!(b.message.as_deref(), Some("Login expirado no WSL.
Abra o Claude Code no WSL para renovar."));
        assert_eq!(b.subtitle, "a@b.c · WSL · WSL desligado");
    }

    #[test]
    fn stopped_suffix() {
        let c = claude_snapshot(vec![claude(vec![wsl()], Ok(usage(9.0, 18.0)), Some("a@b.c"), true)], true);
        assert_eq!(claude_blocks(&c, NOW, 0)[0].subtitle, "a@b.c · WSL · WSL desligado");
        let x = CodexSnapshot { accounts: vec![codex(vec![wsl()], true)], wsl_stopped: true };
        assert!(codex_blocks(&x, NOW, 0)[0].subtitle.ends_with(" · WSL desligado"));
    }
}
