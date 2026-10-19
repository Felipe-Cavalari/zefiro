//! Uso do Claude e do Codex no laço de eventos: serviços, chips e painéis.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::usage::{UsageRow, usage_panel};
use zefiro_integration_clock::windows::utc_offset_seconds;
use zefiro_integration_usage::codex::{CodexService, CodexUsage, Window, effective_percent, window_name};
use zefiro_integration_usage::format::{format_absolute, format_relative, format_short, parse_iso_utc};
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_CODEX};

pub const CODEX_GLYPH: &str = "\u{E943}";

#[derive(Default)]
pub struct UsageRuntime {
    codex: Option<CodexService>,
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default()
}

/// % da janela primária, 0 se ela já reiniciou.
pub fn codex_percent(u: &CodexUsage) -> Option<f32> {
    u.primary.map(|w| effective_percent(&w, now_unix()))
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

fn codex_row(w: &Window, now: i64, offset: i32) -> UsageRow {
    UsageRow { label: window_name(w.window_minutes), percent: Some(effective_percent(w, now)), detail: reset_detail(w.resets_at, w.window_minutes, now, offset) }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

impl Loop {
    /// Liga a leitura do Codex (thread própria, a cada 120 s).
    pub fn usage_start(&mut self) {
        self.usage.codex = CodexService::start(home().join(".codex").join("sessions"), Arc::new(|| window::post_user(USER_CODEX)))
            .map_err(|e| tracing::warn!("uso do Codex indisponível: {e}"))
            .ok();
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

    /// Ao abrir o painel: confere o dado agora.
    pub fn usage_prepare(&self, kind: PanelKind) {
        if kind == PanelKind::Codex
            && let Some(s) = &self.usage.codex
        {
            s.refresh();
        }
    }

    pub fn usage_panel_list(&self, kind: PanelKind, scale: f32) -> DisplayList {
        let tokens = &self.ctx.theme.tokens;
        let (now, offset) = (now_unix(), utc_offset_seconds());
        if kind == PanelKind::Claude {
            return usage_panel("Claude", &[], Some("Carregando…"), "", scale, tokens);
        }
        match &self.ctx.usage.codex {
            Some((_, u)) => {
                let title = u.plan.as_deref().map_or("Codex".to_string(), |p| format!("Codex · {}", capitalized(p)));
                let rows: Vec<UsageRow> = [u.primary, u.secondary].iter().flatten().map(|w| codex_row(w, now, offset)).collect();
                let footer = u.event_at.as_deref().and_then(parse_iso_utc).map(|t| format!("Último uso do Codex: {}", format_short(t, offset))).unwrap_or_default();
                usage_panel(&title, &rows, None, &footer, scale, tokens)
            }
            None => usage_panel("Codex", &[], Some("Nenhum uso do Codex encontrado"), "", scale, tokens),
        }
    }
}
