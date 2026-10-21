//! Estado compartilhado pelas visões.

use std::path::PathBuf;
use std::time::Instant;
use zefiro_core::config::Config;
use zefiro_integration_clock::windows::WindowsClock;
use zefiro_island::model::IslandModel;
use zefiro_platform_windows::compositor::Comp;
use zefiro_platform_windows::gfx::Gfx;
use zefiro_platform_windows::text::DWriteText;

use crate::theme_state::ThemeState;

pub struct AppCtx {
    /// Dono do dispositivo D3D/D2D em que o compositor desenha; precisa viver tanto quanto ele.
    pub gfx: Gfx,
    pub comp: Comp,
    pub text: DWriteText,
    pub theme: ThemeState,
    pub config: Config,
    pub config_path: PathBuf,
    pub themes_dir: PathBuf,
    pub clock: WindowsClock,
    /// Ilha embutida: sessões de mídia, fonte escolhida e estado recolhido.
    pub island: IslandModel,
    /// Som, microfone e brilho (chips da barra, painéis e avisos da ilha).
    pub devices: DeviceState,
    /// Uso do Claude e do Codex (chips e painéis).
    pub usage: UsageState,
    /// Notificações e Não perturbe (chips).
    pub system: SystemState,
    pub epoch: Instant,
}

impl AppCtx {
    /// Segundos desde o início do app (relógio da ilha).
    pub fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
}

/// O que os chips de notificações e Não perturbe mostram.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemState {
    pub notifications: usize,
    /// Não perturbe ativo.
    pub dnd: bool,
}

/// O que os chips de uso mostram: as contas do Claude e do Codex no Windows e no WSL.
#[derive(Debug, Clone, Default)]
pub struct UsageState {
    /// As contas do Claude e quando terminou a última consulta.
    pub claude: Option<zefiro_integration_usage::claude::ClaudeSnapshot>,
    pub codex: Option<zefiro_integration_usage::codex::CodexSnapshot>,
}

impl UsageState {
    /// % do chip do Claude: a maior janela de 5 h entre as contas (o último valor bom numa falha
    /// passageira).
    pub fn claude_chip(&self) -> Option<f32> {
        self.claude.as_ref().and_then(|s| zefiro_integration_usage::claude::chip_percent_accounts(&s.accounts, crate::usage_loop::now_unix()))
    }

    /// % do chip do Codex: a maior janela primária entre as contas.
    pub fn codex_chip(&self) -> Option<f32> {
        self.codex.as_ref().and_then(|s| zefiro_integration_usage::codex::codex_chip_percent(&s.accounts, crate::usage_loop::now_unix()))
    }
}

/// O que os chips de som, microfone e brilho mostram.
#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    pub speaker: Option<zefiro_platform_windows::volume::VolumeState>,
    pub mic: Option<zefiro_platform_windows::volume::VolumeState>,
    /// Um por monitor (nome GDI), na ordem do Windows.
    pub brightness: Vec<zefiro_integration_devices::brightness::MonitorBrightness>,
}

impl DeviceState {
    pub fn brightness_of(&self, device: &str) -> Option<u32> {
        self.brightness.iter().find(|m| m.device == device).and_then(|m| m.percent)
    }
}
