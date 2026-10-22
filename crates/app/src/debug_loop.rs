//! HUD de debug (`Ctrl+Alt+Shift+D`) e exportar diagnóstico (§7 do PLANNING, fase 1g).

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Instant;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionDrawingSurface, SpriteVisual};
use windows::Win32::Foundation::HWND;
use windows::core::Result;
use windows_numerics::Vector2;
use zefiro_bar::hud::{Frames, HudSnapshot, HudWarning, ago, hud_paint};
use zefiro_core::observability::RecentWarnings;
use zefiro_integration_clock::windows::now_local;
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::diagnostics::{self, DwmTiming};
use zefiro_platform_windows::window::{self, WindowKind};

use crate::event_loop::{Loop, TIMER_HUD, USER_UPDATE, USER_BRIGHTNESS, USER_BLUETOOTH, USER_CLAUDE, USER_CODEX, USER_DND, USER_HARDWARE, USER_LYRICS, USER_MEDIA, USER_MIC, USER_NOTIFICATION, USER_POWER, USER_PRIVACY, USER_VISUALIZER, USER_VOLUME, USER_VSCODE, USER_WEATHER};

/// Quantos avisos o HUD mostra.
const HUD_WARNINGS: usize = 4;

/// Integrações na ordem do HUD, com o aviso que cada uma manda à UI.
const INTEGRATIONS: [(u32, &str); 17] = [
    (USER_MEDIA, "Mídia"),
    (USER_VISUALIZER, "Visualizador"),
    (USER_VOLUME, "Som"),
    (USER_MIC, "Microfone"),
    (USER_BRIGHTNESS, "Brilho"),
    (USER_WEATHER, "Clima"),
    (USER_HARDWARE, "Hardware"),
    (USER_LYRICS, "Letras"),
    (USER_NOTIFICATION, "Notificações"),
    (USER_DND, "Não perturbe"),
    (USER_BLUETOOTH, "Bluetooth"),
    (USER_PRIVACY, "Privacidade"),
    (USER_CLAUDE, "Claude"),
    (USER_CODEX, "Codex"),
    (USER_VSCODE, "VS Code"),
    (USER_POWER, "Energia"),
    (USER_UPDATE, "Atualização"),
];

/// Avisos e erros do log, guardados pela camada do `tracing` desde o início do `main`.
static RECENT: OnceLock<RecentWarnings> = OnceLock::new();

pub fn set_recent(recent: RecentWarnings) {
    let _ = RECENT.set(recent);
}

struct HudView {
    hwnd: HWND,
    _target: DesktopWindowTarget,
    surface: CompositionDrawingSurface,
    sprite: SpriteVisual,
}

pub struct DebugRuntime {
    started: Instant,
    /// Último dado de cada integração (pelo código do aviso).
    last: BTreeMap<u32, Instant>,
    hud: Option<HudView>,
    /// Leitura anterior do DWM, para os quadros por segundo.
    previous: Option<(DwmTiming, Instant)>,
}

impl Default for DebugRuntime {
    fn default() -> Self {
        DebugRuntime { started: Instant::now(), last: BTreeMap::new(), hud: None, previous: None }
    }
}

impl Loop {
    /// Carimba a integração que mandou um aviso (barato: um `Instant` num mapa).
    pub fn debug_stamp(&mut self, code: u32) {
        if INTEGRATIONS.iter().any(|(c, _)| *c == code) {
            self.debug.last.insert(code, Instant::now());
        }
    }

    /// O HUD está aberto (a chave das Configurações acende).
    pub fn hud_open(&self) -> bool {
        self.debug.hud.is_some()
    }

    /// `Ctrl+Alt+Shift+D` ou a chave das Configurações: mostra ou esconde o HUD.
    pub fn toggle_hud(&mut self) -> Result<()> {
        if let Some(hud) = self.debug.hud.take() {
            window::kill_timer(TIMER_HUD);
            window::destroy(hud.hwnd);
            tracing::info!("HUD de debug fechado");
            return Ok(());
        }
        let hwnd = window::create(WindowKind::Bar, Rect::new(0.0, 0.0, 1.0, 1.0))?;
        let (target, root) = self.ctx.comp.target(hwnd)?;
        let surface = self.ctx.comp.surface(1.0, 1.0)?;
        let sprite = self.ctx.comp.sprite_for(&surface, 1.0, 1.0)?;
        root.Children()?.InsertAtTop(&sprite)?;
        self.debug.hud = Some(HudView { hwnd, _target: target, surface, sprite });
        self.debug.previous = None;
        tracing::info!("HUD de debug aberto");
        window::set_timer(TIMER_HUD, 1000);
        self.hud_tick()
    }

    /// Uma vez por segundo, só com o HUD aberto.
    pub fn hud_tick(&mut self) -> Result<()> {
        if self.debug.hud.is_none() {
            window::kill_timer(TIMER_HUD);
            return Ok(());
        }
        let snapshot = self.hud_snapshot();
        let Some(bar) = self.bars.iter().find(|b| b.monitor().primary).or(self.bars.first()) else { return Ok(()) };
        let scale = bar.scale();
        let list = hud_paint(&snapshot, scale, &self.ctx.theme.tokens);
        let (w, h) = (list.size.w, list.size.h);
        let mon = bar.monitor().rect;
        let rect = Rect::new((mon.right() - w - 12.0 * scale).round(), (bar.band_bottom() + 8.0 * scale).round(), w, h);
        let Some(hud) = &self.debug.hud else { return Ok(()) };
        self.ctx.comp.draw(&hud.surface, w, h, &list, &self.ctx.text)?;
        hud.sprite.SetSize(Vector2 { X: w, Y: h })?;
        window::set_bounds(hud.hwnd, rect, true);
        Ok(())
    }

    fn hud_snapshot(&mut self) -> HudSnapshot {
        let now = Instant::now();
        let frames = diagnostics::dwm_timing().and_then(|t| {
            let previous = self.debug.previous.replace((t, now));
            let (p, at) = previous?;
            let secs = now.duration_since(at).as_secs_f32().max(0.001);
            Some(Frames { per_second: t.frames.saturating_sub(p.frames) as f32 / secs, refresh_hz: t.refresh_hz, dropped: t.dropped.saturating_sub(p.dropped) })
        });
        let (private, ws) = diagnostics::process_memory().unwrap_or_default();
        let integrations = INTEGRATIONS.iter().map(|(code, name)| (name.to_string(), self.debug.last.get(code).map(|t| now.duration_since(*t).as_secs_f64()))).collect();
        let unix_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let warnings = RECENT.get().map(RecentWarnings::list).unwrap_or_default().into_iter().rev().take(HUD_WARNINGS)
            .map(|w| HudWarning {
                clock: ago(unix_now.saturating_sub(w.at_unix_s) as f64),
                error: w.error,
                source: w.target.rsplit("::").next().unwrap_or(&w.target).to_string(),
                message: w.message,
            })
            .collect();
        let mb = |b: u64| b as f32 / (1024.0 * 1024.0);
        HudSnapshot { frames, private_mb: mb(private), working_set_mb: mb(ws), uptime_s: self.debug.started.elapsed().as_secs(), integrations, warnings }
    }

    /// Item "Exportar diagnóstico": zip em Downloads com logs, crashes, config e `info.txt`, sem dados
    /// pessoais, e o Explorer aberto com ele selecionado.
    pub fn export_diagnostics(&mut self) -> Result<()> {
        let t = now_local();
        let when = zefiro_core::diagnostics::dos_datetime(t.date.year as u16, t.date.month as u16, t.date.day as u16, t.hour as u16, t.minute as u16, t.second as u16);
        let name = format!("zefiro-diagnostico-{:04}{:02}{:02}-{:02}{:02}{:02}.zip", t.date.year, t.date.month, t.date.day, t.hour, t.minute, t.second);
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        let user = std::env::var("USERNAME").unwrap_or_default();
        let config = std::fs::read_to_string(&self.ctx.config_path).ok();
        let zip = zefiro_core::diagnostics::diagnostic_zip(&zefiro_core::observability::default_log_dir(), config.as_deref(), &self.diagnostic_info(), &home, &user, when);
        let dir = diagnostics::downloads_dir().unwrap_or_else(std::env::temp_dir);
        let path = dir.join(name);
        match std::fs::write(&path, &zip) {
            Ok(()) => {
                tracing::info!(bytes = zip.len(), "diagnóstico exportado");
                diagnostics::reveal_file(&path);
            }
            Err(e) => tracing::error!("não consegui gravar o diagnóstico: {e}"),
        }
        Ok(())
    }

    fn diagnostic_info(&self) -> String {
        let c = &self.ctx.config;
        let (private, ws) = diagnostics::process_memory().unwrap_or_default();
        let monitors = self.bars.iter().enumerate().map(|(i, b)| {
            let m = b.monitor();
            format!("  {}: {}×{} em ({}, {}), DPI {}{}", i + 1, m.rect.w, m.rect.h, m.rect.x, m.rect.y, m.dpi, if m.primary { ", principal" } else { "" })
        }).collect::<Vec<_>>().join("\n");
        let integrations = INTEGRATIONS.iter().filter_map(|(code, name)| self.debug.last.get(code).map(|t| format!("  {name}: {}", ago(t.elapsed().as_secs_f64())))).collect::<Vec<_>>().join("\n");
        format!(
            "Zefiro {}\n{}\nLigado há {} s\nMemória: {:.1} MB privada, {:.1} MB working set\nTema: {}\nBarra: {} / {}, layout {:?}\nIlha: {} ({:?})\nMonitores:\n{monitors}\nÚltimo dado de cada integração:\n{integrations}\n",
            env!("CARGO_PKG_VERSION"),
            diagnostics::windows_version(),
            self.debug.started.elapsed().as_secs(),
            private as f64 / 1048576.0,
            ws as f64 / 1048576.0,
            c.theme,
            c.bar.height.as_str(),
            c.bar.format.as_str(),
            c.bar.layout_mode,
            if c.island.enabled { "ligada" } else { "desligada" },
            c.island.engine,
        )
    }

    pub fn debug_shutdown(&mut self) {
        if let Some(hud) = self.debug.hud.take() {
            window::destroy(hud.hwnd);
        }
        diagnostics::unregister_hud_hotkey();
    }
}
