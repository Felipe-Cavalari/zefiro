#![windows_subsystem = "windows"]

mod app;
mod autohide_loop;
mod bar_view;
mod chat_loop;
mod config_worker;
mod debug_loop;
mod editor_view;
mod event_loop;
mod devices_loop;
mod island;
mod island_loop;
mod island_view;
mod panel_view;
mod power_loop;
mod robustness;
mod system_loop;
mod tab_strip;
mod tray_loop;
mod update_loop;
mod usage_loop;
mod terminal_loop;
mod vscode_loop;
mod theme_state;

use std::time::Instant;
use zefiro_core::config::{default_config_dir, load_or_create};
use zefiro_core::observability::{default_log_dir, init_logging, install_panic_hook};
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::{compositor::Comp, gfx::Gfx, system, text::DWriteText, window};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let started = Instant::now();
    // Antes de qualquer janela: DPI por monitor (Per-Monitor v2).
    let dpi_aware = system::enable_per_monitor_dpi();
    let log_dir = default_log_dir();
    install_panic_hook(log_dir.clone(), VERSION);
    let _guard = init_logging(&log_dir, "info").ok();
    if let Some(guard) = &_guard {
        debug_loop::set_recent(guard.recent.clone());
    }
    if !dpi_aware {
        tracing::warn!("não consegui ligar DPI por monitor; a barra pode ficar borrada fora de 100%");
    }
    if !system::single_instance("Local\\zefiro-bar") {
        tracing::info!("zefiro já está rodando; saindo");
        return;
    }
    if let Err(e) = run(started) {
        tracing::error!(error = %e, "zefiro encerrou com erro");
    }
}

fn run(started: Instant) -> windows::core::Result<()> {
    let config_dir = default_config_dir();
    let config_path = config_dir.join("config.toml");
    let themes_dir = config_dir.join("themes");
    let config = load_or_create(&config_path).unwrap_or_else(|e| {
        tracing::error!(line = ?e.line, "config.toml inválido: {}; usando padrões", e.message);
        zefiro_core::config::Config::default()
    });
    let (mut theme, errors) = theme_state::ThemeState::load(&config.theme, &themes_dir);
    theme.acrylic = config.acrylic;
    for e in errors {
        tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}", e.message);
    }
    window::register_classes()?;
    window::create(window::WindowKind::Message, Rect::default())?;
    let gfx = Gfx::new()?;
    let comp = Comp::new(&gfx)?;
    let text = DWriteText::new(gfx.dwrite.clone(), &theme.theme.font.family);
    let ctx = app::AppCtx { gfx, comp, text, theme, config, config_path, themes_dir, clock: zefiro_integration_clock::windows::WindowsClock, island: Default::default(), devices: Default::default(), usage: Default::default(), system: Default::default(), epoch: started };

    let mut bars = Vec::new();
    for m in robustness::bar_monitors(&ctx.config) {
        bars.push(bar_view::BarView::create(&ctx, &m)?);
    }
    tracing::info!(ms = started.elapsed().as_millis() as u64, monitors = bars.len(), "primeiro quadro da barra");
    for b in &mut bars {
        b.register_appbar(&ctx);
    }
    let cfg_path = ctx.config_path.clone();
    let _watcher = zefiro_core::watch::watch_paths(&[config_dir.clone(), ctx.themes_dir.clone()], std::time::Duration::from_millis(150), move |paths| {
        for p in paths {
            if p == cfg_path {
                window::post_user(event_loop::USER_CONFIG_CHANGED);
            } else if p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("island-state.ini")) {
                window::post_user(event_loop::USER_ISLAND_ACCENT);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) && p.parent().is_some_and(|d| d.ends_with("themes")) {
                window::post_user(event_loop::USER_THEMES_CHANGED);
            }
        }
    })
    .map_err(|e| tracing::warn!("observador de arquivos indisponível: {e}"))
    .ok();
    event_loop::run(ctx, bars)
}
