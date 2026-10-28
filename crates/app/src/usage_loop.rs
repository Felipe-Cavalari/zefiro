//! Uso do Claude, do Codex e do Grok no laço de eventos: serviços, chips e painéis.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::usage::{RefreshButton, UsageBlock, UsageRow, refresh_button_rect, usage_panel};
use zefiro_integration_clock::windows::utc_offset_seconds;
use zefiro_integration_usage::claude::{ClaudeSnapshot, Limit, UsageError, can_refresh};
use zefiro_integration_usage::codex::{CodexService, CodexSnapshot, Window, effective_percent, window_name};
use zefiro_integration_usage::grok::{GrokSnapshot, format_usd};
use zefiro_integration_usage::origins::{OriginKind, many_distros, origins_label};
use zefiro_integration_usage::format::{format_absolute, format_clock, format_relative, format_short, parse_iso_utc};
use zefiro_platform_windows::claude::ClaudeService;
use zefiro_platform_windows::grok_usage::GrokUsageService;
use zefiro_platform_windows::{window, wsl};
use zefiro_platform::geometry::Point;
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_CLAUDE, USER_CODEX, USER_GROK};

pub const CODEX_GLYPH: &str = "\u{E943}";
pub const CLAUDE_GLYPH: &str = "\u{E945}";
/// Robô (Segoe Fluent Icons).
pub const GROK_GLYPH: &str = "\u{E99A}";

#[derive(Default)]
pub struct UsageRuntime {
    codex: Option<CodexService>,
    claude: Option<ClaudeService>,
    grok: Option<GrokUsageService>,
    /// O botão de atualizar foi tocado e a resposta ainda não chegou.
    claude_refreshing: bool,
    codex_refreshing: bool,
    grok_refreshing: bool,
    /// O mouse está sobre o botão de atualizar do painel aberto.
    refresh_hover: bool,
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
        UsageError::NotInstalled => format!("Claude Code não encontrado no {origin}"),
        UsageError::Network => "Sem conexão.\nTento de novo em 5 min.".into(),
        UsageError::UnknownFormat => "Formato de resposta desconhecido".into(),
        UsageError::Http(status) => format!("O serviço respondeu {status}.\nTento de novo em 5 min."),
    }
}

/// A pausa mais curta entre as contas que receberam 429 e ainda não voltaram.
fn paused_until(s: &ClaudeSnapshot, now: i64) -> Option<i64> {
    s.accounts.iter().filter_map(|a| a.data.retry_at).filter(|t| *t > now).min()
}

/// Rodapé do painel do Claude: atualizando, em pausa depois de um 429 ou "Atualizado há".
fn claude_footer(s: &ClaudeSnapshot, now: i64, offset: i32, refreshing: bool) -> String {
    if refreshing {
        return "Atualizando…".into();
    }
    match paused_until(s, now) {
        Some(t) => format!("Muitas consultas. Tento de novo às {}.", format_clock(t, offset)),
        None => s.at.map(updated_ago).unwrap_or_default(),
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
            // Conta lembrada de uma distro parada, ou em pausa depois de um 429: o último valor bom,
            // sem mensagem de erro (o rodapé diz quando tenta de novo).
            let usage = match &a.data.result {
                Some(Ok(u)) => Ok(Some(u)),
                Some(Err(e)) if (a.stopped || *e == UsageError::Http(429)) && a.data.last_ok.is_some() => Ok(a.data.last_ok.as_ref()),
                Some(Err(UsageError::Http(429))) if a.data.retry_at.is_some_and(|t| t > now) => {
                    Err(format!("Muitas consultas.\nTento de novo às {}.", a.data.retry_at.map(|t| format_clock(t, offset)).unwrap_or_default()))
                }
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
            // Com uma conta só, o título não diz de onde veio o uso.
            if s.accounts.len() == 1 {
                let origins = origins_label(&a.origins, many);
                subtitle = if subtitle.is_empty() { origins } else { format!("{subtitle} · {origins}") };
            }
            if a.stopped {
                subtitle.push_str(STOPPED_SUFFIX);
            }
            let rows = [u.primary, u.secondary].iter().flatten().map(|w| codex_row(w, now, offset)).collect();
            block(title, subtitle, rows, None)
        })
        .collect()
}

/// `origin`: "Windows" ou "WSL", onde rodar o `grok login`.
fn grok_message(e: UsageError, origin: &str) -> String {
    match e {
        UsageError::Missing | UsageError::Expired => format!("Faça login no Grok Build:\nrode `grok login` no {origin}."),
        UsageError::NotInstalled => format!("Grok Build não encontrado no {origin}"),
        UsageError::Network | UsageError::Http(_) => "O Grok Build não respondeu.\nTento de novo em 5 min.".into(),
        UsageError::UnknownFormat => "Formato de resposta desconhecido".into(),
    }
}

/// Um bloco por conta do Grok: a cota do período (semanal no SuperGrok) e, no subtítulo, os
/// créditos extras quando há saldo.
fn grok_blocks(s: &GrokSnapshot, now: i64, offset: i32) -> Vec<UsageBlock> {
    if s.accounts.is_empty() {
        let message = if s.wsl_stopped { WSL_STOPPED } else { "Grok Build não encontrado.\nInstale e rode `grok login`." };
        return vec![block("Grok".into(), String::new(), Vec::new(), Some(message.into()))];
    }
    let many = many_distros(&s.accounts);
    s.accounts
        .iter()
        .map(|a| {
            let title = a.data.plan.as_deref().map_or("Grok".to_string(), |p| format!("Grok · {p}"));
            let origins = origins_label(&a.origins, many);
            let mut subtitle = a.data.email.as_deref().map_or(origins.clone(), |e| format!("{e} · {origins}"));
            let renew_at = if a.origins.contains(&OriginKind::Windows) { "Windows" } else { "WSL" };
            // Conta lembrada de uma distro parada ou falha passageira: o último valor bom.
            let usage = match &a.data.result {
                Some(Ok(u)) => Ok(Some(u)),
                Some(Err(e)) if (a.stopped || *e == UsageError::Network) && a.data.last_ok.is_some() => Ok(a.data.last_ok.as_ref()),
                Some(Err(e)) => Err(grok_message(*e, renew_at)),
                None => Ok(None),
            };
            if let Ok(Some(u)) = &usage
                && let Some(cents) = u.prepaid_cents.filter(|c| *c > 0)
            {
                subtitle.push_str(&format!(" · créditos extras {}", format_usd(cents)));
            }
            if a.stopped {
                subtitle.push_str(STOPPED_SUFFIX);
            }
            match usage {
                Err(message) => block(title, subtitle, Vec::new(), Some(message)),
                Ok(None) => block(title, subtitle, Vec::new(), Some("Carregando…".into())),
                Ok(Some(u)) => {
                    let minutes = u.period_minutes.unwrap_or(10080);
                    let label = if minutes == 10080 { "Semana" } else { "Período" };
                    let rows = u.percent.map(|percent| UsageRow { label: label.into(), percent: Some(percent), detail: u.period_end.map(|t| reset_detail(t, minutes, now, offset)).unwrap_or_default() }).into_iter().collect();
                    block(title, subtitle, rows, None)
                }
            }
        })
        .collect()
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

impl Loop {
    /// Liga ou desliga o Claude, o Codex e o Grok conforme os chips estão na barra: sem o chip, o
    /// `/usage` do Claude Code e o `grok` não rodam, e a pasta do Codex não é varrida.
    pub fn usage_services(&mut self) {
        match (self.ctx.config.bar.uses("grok"), self.usage.grok.is_some()) {
            (true, false) => {
                tracing::info!("uso do Grok: serviço ligado");
                self.usage.grok = GrokUsageService::start(Arc::new(|| window::post_user(USER_GROK))).map_err(|e| tracing::warn!("uso do Grok indisponível: {e}")).ok();
            }
            (false, true) => {
                tracing::info!("uso do Grok: serviço desligado (chip fora da barra)");
                self.usage.grok = None;
                self.ctx.usage.set_grok(None, now_unix());
            }
            _ => {}
        }
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
        self.usage.codex_refreshing = false;
        let latest = self.usage.codex.as_ref().and_then(CodexService::latest);
        if self.ctx.usage.set_codex(latest, now_unix()) {
            self.apply_bars(true)?;
        }
        if self.usage_panel_open(PanelKind::Codex) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Chega a cada consulta (a cada 5 min, ao abrir o painel e no botão de atualizar): a barra só é
    /// refeita se o número do chip mudou; o painel aberto é redesenhado sempre (rodapé).
    pub fn usage_claude_changed(&mut self) -> Result<()> {
        self.usage.claude_refreshing = false;
        let latest = self.usage.claude.as_ref().and_then(ClaudeService::latest);
        if self.ctx.usage.set_claude(latest, now_unix()) {
            self.apply_bars(true)?;
        }
        if self.usage_panel_open(PanelKind::Claude) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Chega a cada consulta do Grok: como o Claude.
    pub fn usage_grok_changed(&mut self) -> Result<()> {
        self.usage.grok_refreshing = false;
        let latest = self.usage.grok.as_ref().and_then(GrokUsageService::latest);
        if self.ctx.usage.set_grok(latest, now_unix()) {
            self.apply_bars(true)?;
        }
        if self.usage_panel_open(PanelKind::Grok) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Algum painel de uso aberto (o botão de atualizar e o hover valem nele).
    fn usage_any_open(&self) -> bool {
        [PanelKind::Claude, PanelKind::Codex, PanelKind::Grok].into_iter().any(|k| self.usage_panel_open(k))
    }

    /// Ao abrir o painel: confere o dado agora.
    pub fn usage_prepare(&mut self, kind: PanelKind) {
        if kind == PanelKind::Grok
            && let Some(s) = &self.usage.grok
        {
            s.refresh();
        }
        self.usage.refresh_hover = false;
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

    /// Botão de atualizar dos painéis do Claude, do Codex e do Grok. `true` se o clique foi nele.
    pub fn usage_click(&mut self, p: Point, scale: f32) -> Result<bool> {
        if !refresh_button_rect(scale).contains(p) {
            return Ok(false);
        }
        if self.usage_panel_open(PanelKind::Grok) {
            if !self.usage.grok_refreshing
                && let Some(service) = &self.usage.grok
            {
                service.refresh_now();
                self.usage.grok_refreshing = true;
                self.redraw_open_panel()?;
            }
            return Ok(true);
        }
        if self.usage_panel_open(PanelKind::Claude) {
            if self.claude_refresh_enabled()
                && let Some(service) = &self.usage.claude
            {
                service.refresh_now();
                self.usage.claude_refreshing = true;
                self.redraw_open_panel()?;
            }
            return Ok(true);
        }
        if self.usage_panel_open(PanelKind::Codex) {
            // O Codex lê arquivos locais: sem limite de consultas, o botão só espera a resposta.
            if !self.usage.codex_refreshing
                && let Some(service) = &self.usage.codex
            {
                service.refresh();
                self.usage.codex_refreshing = true;
                self.redraw_open_panel()?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Mouse sobre o painel de uso: o botão de atualizar acende sob ele (só redesenha quando muda).
    pub fn usage_move(&mut self, p: Point, scale: f32) -> Result<()> {
        if !self.usage_any_open() {
            return Ok(());
        }
        self.usage_hover(refresh_button_rect(scale).contains(p))
    }

    /// O mouse saiu do painel: o botão apaga.
    pub fn usage_leave(&mut self) -> Result<()> {
        self.usage_hover(false)
    }

    fn usage_hover(&mut self, over: bool) -> Result<()> {
        if over == self.usage.refresh_hover {
            return Ok(());
        }
        self.usage.refresh_hover = over;
        if self.usage_any_open() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    fn claude_refresh_enabled(&self) -> bool {
        !self.usage.claude_refreshing && self.ctx.usage.claude.as_ref().is_some_and(|s| can_refresh(s, now_unix()))
    }

    pub fn usage_panel_list(&self, kind: PanelKind, scale: f32) -> DisplayList {
        let (tokens, text) = (&self.ctx.theme.tokens, &self.ctx.text);
        let (now, offset) = (now_unix(), utc_offset_seconds());
        let hovered = self.usage.refresh_hover;
        if kind == PanelKind::Claude {
            let refresh = Some(RefreshButton { enabled: self.claude_refresh_enabled(), hovered });
            return match &self.ctx.usage.claude {
                None => usage_panel(&[block("Claude".into(), String::new(), Vec::new(), Some("Carregando…".into()))], "", refresh, scale, tokens, text),
                Some(s) => usage_panel(&claude_blocks(s, now, offset), &claude_footer(s, now, offset, self.usage.claude_refreshing), refresh, scale, tokens, text),
            };
        }
        if kind == PanelKind::Grok {
            let refresh = Some(RefreshButton { enabled: !self.usage.grok_refreshing, hovered });
            return match &self.ctx.usage.grok {
                None => usage_panel(&[block("Grok".into(), String::new(), Vec::new(), Some("Carregando…".into()))], "", refresh, scale, tokens, text),
                Some(s) => {
                    let footer = if self.usage.grok_refreshing { "Atualizando…".to_string() } else { s.at.map(updated_ago).unwrap_or_default() };
                    usage_panel(&grok_blocks(s, now, offset), &footer, refresh, scale, tokens, text)
                }
            };
        }
        let blocks = codex_blocks(self.ctx.usage.codex.as_ref().unwrap_or(&CodexSnapshot::default()), now, offset);
        usage_panel(&blocks, "", Some(RefreshButton { enabled: !self.usage.codex_refreshing, hovered }), scale, tokens, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use zefiro_integration_usage::claude::{ClaudeEntry, ClaudeSnapshot, ClaudeUsage};
    use zefiro_integration_usage::codex::{CodexEntry, CodexSnapshot, CodexUsage};
    use zefiro_integration_usage::grok::{GrokEntry, GrokUsage};
    use zefiro_integration_usage::origins::Account;

    const NOW: i64 = 1_792_000_000;

    fn claude(origins: Vec<OriginKind>, result: std::result::Result<ClaudeUsage, UsageError>, email: Option<&str>, stopped: bool) -> Account<ClaudeEntry> {
        let data = ClaudeEntry { email: email.map(String::from), plan: Some("Pro".into()), result: Some(result), ..Default::default() };
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

    fn limited(last_ok: Option<ClaudeUsage>) -> Account<ClaudeEntry> {
        let mut a = claude(vec![OriginKind::Windows], Err(UsageError::Http(429)), Some("a@b.c"), false);
        a.data.last_ok = last_ok;
        a.data.retry_at = Some(NOW + 600);
        a
    }

    #[test]
    fn rate_limited_account_keeps_last_values() {
        let b = &claude_blocks(&claude_snapshot(vec![limited(Some(usage(9.0, 18.0)))], false), NOW, 0)[0];
        assert_eq!(b.message, None);
        assert_eq!(b.rows.len(), 2);
    }

    #[test]
    fn rate_limited_without_values_says_when() {
        let b = &claude_blocks(&claude_snapshot(vec![limited(None)], false), NOW, 0)[0];
        assert_eq!(b.message.as_deref(), Some(format!("Muitas consultas.\nTento de novo às {}.", format_clock(NOW + 600, 0)).as_str()));
    }

    #[test]
    fn claude_footer_rules() {
        let at = Some(std::time::Instant::now());
        let ok = ClaudeSnapshot { at, ..claude_snapshot(vec![claude(vec![OriginKind::Windows], Ok(usage(9.0, 18.0)), None, false)], false) };
        assert_eq!(claude_footer(&ok, NOW, 0, false), "Atualizado agora");
        assert_eq!(claude_footer(&ok, NOW, 0, true), "Atualizando…");
        let paused = ClaudeSnapshot { at, ..claude_snapshot(vec![limited(None)], false) };
        assert_eq!(claude_footer(&paused, NOW, 0, false), format!("Muitas consultas. Tento de novo às {}.", format_clock(NOW + 600, 0)));
        assert_eq!(claude_footer(&claude_snapshot(Vec::new(), false), NOW, 0, false), "");
    }

    #[test]
    fn codex_single_account_shows_its_origins() {
        let one = CodexSnapshot { accounts: vec![codex(vec![OriginKind::Windows, wsl()], false)], wsl_stopped: false };
        assert!(codex_blocks(&one, NOW, 0)[0].subtitle.ends_with(" · Windows · WSL"), "{}", codex_blocks(&one, NOW, 0)[0].subtitle);
    }

    fn grok(origins: Vec<OriginKind>, result: std::result::Result<GrokUsage, UsageError>, stopped: bool) -> Account<GrokEntry> {
        let last_ok = result.as_ref().ok().cloned();
        let data = GrokEntry { email: Some("a@b.c".into()), plan: Some("SuperGrok".into()), result: Some(result), last_ok };
        Account { key: format!("{origins:?}"), origins, stopped, data }
    }

    fn week(percent: f32, cents: i64) -> GrokUsage {
        GrokUsage { percent: Some(percent), period_end: Some(NOW + 2 * 86_400), period_minutes: Some(10080), plan: Some("SuperGrok".into()), prepaid_cents: Some(cents) }
    }

    #[test]
    fn grok_block_shows_the_week_and_extra_credits() {
        let s = GrokSnapshot { accounts: vec![grok(vec![wsl()], Ok(week(25.0, 400)), false)], wsl_stopped: false, at: None };
        let b = &grok_blocks(&s, NOW, 0)[0];
        assert_eq!(b.title, "Grok · SuperGrok");
        assert_eq!(b.subtitle, "a@b.c · WSL · créditos extras US$ 4,00");
        assert_eq!(b.rows.len(), 1);
        assert_eq!((b.rows[0].label.as_str(), b.rows[0].percent), ("Semana", Some(25.0)));
        assert!(b.rows[0].detail.starts_with("reinicia "), "{}", b.rows[0].detail);
        let none = GrokSnapshot { accounts: vec![grok(vec![OriginKind::Windows], Ok(week(25.0, 0)), false)], wsl_stopped: false, at: None };
        assert_eq!(grok_blocks(&none, NOW, 0)[0].subtitle, "a@b.c · Windows", "sem saldo, sem créditos");
    }

    #[test]
    fn grok_messages() {
        let lost = GrokSnapshot { accounts: vec![grok(vec![wsl()], Err(UsageError::Missing), false)], wsl_stopped: false, at: None };
        assert_eq!(grok_blocks(&lost, NOW, 0)[0].message.as_deref(), Some("Faça login no Grok Build:\nrode `grok login` no WSL."));
        let empty = grok_blocks(&GrokSnapshot::default(), NOW, 0);
        assert_eq!(empty[0].message.as_deref(), Some("Grok Build não encontrado.\nInstale e rode `grok login`."));
        let stopped = grok_blocks(&GrokSnapshot { wsl_stopped: true, ..Default::default() }, NOW, 0);
        assert_eq!(stopped[0].message.as_deref(), Some(WSL_STOPPED));
        let mut flaky = grok(vec![wsl()], Ok(week(30.0, 0)), false);
        flaky.data.result = Some(Err(UsageError::Network));
        let b = &grok_blocks(&GrokSnapshot { accounts: vec![flaky], ..Default::default() }, NOW, 0)[0];
        assert_eq!((b.message.as_deref(), b.rows[0].percent), (None, Some(30.0)), "falha passageira mostra o último valor");
    }

    #[test]
    fn stopped_suffix() {
        let c = claude_snapshot(vec![claude(vec![wsl()], Ok(usage(9.0, 18.0)), Some("a@b.c"), true)], true);
        assert_eq!(claude_blocks(&c, NOW, 0)[0].subtitle, "a@b.c · WSL · WSL desligado");
        let x = CodexSnapshot { accounts: vec![codex(vec![wsl()], true)], wsl_stopped: true };
        assert!(codex_blocks(&x, NOW, 0)[0].subtitle.ends_with(" · WSL desligado"));
    }
}
