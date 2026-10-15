#![windows_subsystem = "windows"]

mod app;
mod bar_view;
mod theme_state;

use std::time::Instant;
use zefiro_core::config::{default_config_dir, load_or_create};
use zefiro_core::observability::{default_log_dir, init_logging, install_panic_hook};
use zefiro_platform::geometry::Rect;
use zefiro_platform_windows::{compositor::Comp, gfx::Gfx, monitors, system, text::DWriteText, window};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let started = Instant::now();
    let log_dir = default_log_dir();
    install_panic_hook(log_dir.clone(), VERSION);
    let _guard = init_logging(&log_dir, "info").ok();
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
    let (theme, errors) = theme_state::ThemeState::load(&config.theme, &themes_dir);
    for e in errors {
        tracing::warn!(source = %e.source, line = ?e.line, "tema com erro: {}", e.message);
    }
    window::register_classes()?;
    window::create(window::WindowKind::Message, Rect::default())?;
    let gfx = Gfx::new()?;
    let comp = Comp::new(&gfx)?;
    let text = DWriteText::new(gfx.dwrite.clone(), &theme.theme.font.family);
    let ctx = app::AppCtx { gfx, comp, text, theme, config, config_path, themes_dir, clock: zefiro_integration_clock::windows::WindowsClock };

    let mut bars = Vec::new();
    for m in monitors::list() {
        bars.push(bar_view::BarView::create(&ctx, &m)?);
    }
    tracing::info!(ms = started.elapsed().as_millis() as u64, monitors = bars.len(), "primeiro quadro da barra");
    for b in &mut bars {
        b.register_appbar(&ctx);
    }
    while window::pump() {
        for e in window::take_events() {
            if matches!(e, window::WinEvent::Quit) {
                return Ok(());
            }
        }
    }
    Ok(())
}
