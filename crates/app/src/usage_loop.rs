//! Uso do Claude e do Codex no laço de eventos: serviços, chips e painéis.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::usage::{UsageRow, usage_panel};
use zefiro_integration_clock::windows::utc_offset_seconds;
use zefiro_integration_usage::claude::{Limit, UsageError};
use zefiro_integration_usage::codex::{CodexService, Window, effective_percent, window_name};
use zefiro_integration_usage::origins::{Discovery, Origin, OriginKind};
use zefiro_integration_usage::format::{format_absolute, format_relative, format_short, parse_iso_utc};
use zefiro_platform_windows::claude::ClaudeService;
use zefiro_platform_windows::window;
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

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default()
}


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

fn claude_message(e: UsageError) -> String {
    match e {
        UsageError::Expired => "Login do Claude Code expirado.\nAbra o Claude Code para renovar.".into(),
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
                self.usage.claude = ClaudeService::start(home().join(".claude").join(".credentials.json"), Arc::new(|| window::post_user(USER_CLAUDE)))
                    .map_err(|e| tracing::warn!("uso do Claude indisponível: {e}"))
                    .ok();
            }
            (false, true) => {
                tracing::info!("uso do Claude: serviço desligado (chip fora da barra)");
                self.usage.claude = None;
                self.ctx.usage.claude = None;
            }
            _ => {}
        }
        match (codex, self.usage.codex.is_some()) {
            (true, false) => {
                tracing::info!("uso do Codex: serviço ligado");
                let windows = Origin { kind: OriginKind::Windows, home: home() };
                let discover = Arc::new(move || Discovery { origins: vec![windows.clone()], ..Default::default() });
                self.usage.codex = CodexService::start(discover, Arc::new(|| window::post_user(USER_CODEX)))
                    .map_err(|e| tracing::warn!("uso do Codex indisponível: {e}"))
                    .ok();
            }
            (false, true) => {
                tracing::info!("uso do Codex: serviço desligado (chip fora da barra)");
                self.usage.codex = None;
                self.ctx.usage.codex = None;
            }
            _ => {}
        }
    }

    fn usage_panel_open(&self, kind: PanelKind) -> bool {
        self.controller.open().is_some_and(|o| o.kind == kind)
    }

    pub fn usage_codex_changed(&mut self) -> Result<()> {
        self.ctx.usage.codex = self.usage.codex.as_ref().and_then(CodexService::latest);
        self.apply_bars(true)?;
        if self.usage_panel_open(PanelKind::Codex) {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    /// Chega a cada consulta (a cada 2 min): a barra só é refeita se o número do chip mudou; o
    /// painel aberto é redesenhado sempre (rodapé "Atualizado há").
    pub fn usage_claude_changed(&mut self) -> Result<()> {
        let before = self.ctx.usage.claude_chip();
        self.ctx.usage.claude = self.usage.claude.as_ref().and_then(ClaudeService::latest);
        if let Some((_, Ok(u))) = &self.ctx.usage.claude {
            self.ctx.usage.claude_last_ok = Some(u.clone());
        }
        if self.ctx.usage.claude_chip() != before {
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
        let tokens = &self.ctx.theme.tokens;
        let (now, offset) = (now_unix(), utc_offset_seconds());
        if kind == PanelKind::Claude {
            return match &self.ctx.usage.claude {
                None => usage_panel("Claude", &[], Some("Carregando…"), "", scale, tokens),
                Some((at, Err(e))) => usage_panel("Claude", &[], Some(&claude_message(*e)), &updated_ago(*at), scale, tokens),
                Some((at, Ok(u))) => {
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
                    usage_panel("Claude", &rows, None, &updated_ago(*at), scale, tokens)
                }
            };
        }
        match self.ctx.usage.codex.as_ref().and_then(|s| s.accounts.first()).map(|a| &a.data.usage) {
            Some(u) => {
                let title = u.plan.as_deref().map_or("Codex".to_string(), |p| format!("Codex · {}", capitalized(p)));
                let rows: Vec<UsageRow> = [u.primary, u.secondary].iter().flatten().map(|w| codex_row(w, now, offset)).collect();
                let footer = u.event_at.as_deref().and_then(parse_iso_utc).map(|t| format!("Último uso do Codex: {}", format_short(t, offset))).unwrap_or_default();
                usage_panel(&title, &rows, None, &footer, scale, tokens)
            }
            None => usage_panel("Codex", &[], Some("Nenhum uso do Codex encontrado"), "", scale, tokens),
        }
    }
}
