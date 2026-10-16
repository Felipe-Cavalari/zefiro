//! Som, microfone e brilho no laço de eventos: serviços, chips, painéis (sliders, mudo, troca de
//! dispositivo), roda do mouse nos chips e o aviso na ilha.

use std::sync::Arc;
use windows::core::Result;
use zefiro_bar::controller::PanelKind;
use zefiro_bar::devices::{DeviceAction, DeviceItem, DevicePanel, ICON_BRIGHTNESS, SliderRow, device_panel, mic_icon, speaker_icon};
use zefiro_integration_devices::level::{slider_percent, wheel};
use zefiro_platform::geometry::Point;
use zefiro_platform_windows::audio_devices::{self, AudioDevice, Flow};
use zefiro_platform_windows::brightness::BrightnessService;
use zefiro_platform_windows::volume::VolumeWatcher;
use zefiro_platform_windows::window;
use zefiro_ui::paint::DisplayList;

use crate::event_loop::{Loop, USER_BRIGHTNESS, USER_MIC, USER_VOLUME};

const OUT: &str = "out";
const IN: &str = "in";
/// Passo da roda: 2 % no volume, 5 % no brilho.
const VOLUME_STEP: u32 = 2;
const BRIGHTNESS_STEP: u32 = 5;

#[derive(Default)]
pub struct DevicesRuntime {
    speaker: Option<VolumeWatcher>,
    mic: Option<VolumeWatcher>,
    brightness: Option<BrightnessService>,
    /// Painel aberto (som, microfone ou brilho): desenho, trilhos e áreas de clique.
    panel: Option<DevicePanel>,
    /// Dispositivos listados ao abrir o painel (enumerar a cada quadro do arraste seria caro).
    devices: Vec<AudioDevice>,
    default_device: Option<String>,
    /// Slider sendo arrastado (id da linha).
    drag: Option<String>,
}

fn pct(level: f32) -> u32 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u32
}

impl Loop {
    /// Liga os observadores de volume (saída e entrada) e o brilho por DDC/CI. Cada um tem sua
    /// thread e avisa a UI só quando algo muda.
    pub fn devices_start(&mut self) {
        let d = &mut self.devices;
        d.speaker = VolumeWatcher::start(Flow::Output, Arc::new(|| window::post_user(USER_VOLUME))).map_err(|e| tracing::warn!("volume indisponível: {e}")).ok();
        d.mic = VolumeWatcher::start(Flow::Input, Arc::new(|| window::post_user(USER_MIC))).map_err(|e| tracing::warn!("microfone indisponível: {e}")).ok();
        d.brightness = BrightnessService::start(Arc::new(|| window::post_user(USER_BRIGHTNESS))).map_err(|e| tracing::warn!("brilho indisponível: {e}")).ok();
        self.ctx.devices.speaker = d.speaker.as_ref().and_then(VolumeWatcher::state);
        self.ctx.devices.mic = d.mic.as_ref().and_then(VolumeWatcher::state);
    }

    /// Monitores mudaram: o brilho reabre os handles do DDC/CI.
    pub fn devices_displays_changed(&self) {
        if let Some(b) = &self.devices.brightness {
            b.refresh();
        }
    }

    fn devices_panel_open(&self) -> Option<(usize, PanelKind)> {
        self.controller.open().filter(|o| matches!(o.kind, PanelKind::Audio | PanelKind::Mic | PanelKind::Brightness)).map(|o| (o.monitor, o.kind))
    }

    /// Chips e painel aberto refletem o estado novo.
    fn devices_redraw(&mut self) -> Result<()> {
        self.apply_bars(true)?;
        if self.devices_panel_open().is_some() {
            self.redraw_open_panel()?;
        }
        Ok(())
    }

    pub fn devices_volume_changed(&mut self, flow: Flow) -> Result<()> {
        let watcher = if flow == Flow::Output { &self.devices.speaker } else { &self.devices.mic };
        let state = watcher.as_ref().and_then(VolumeWatcher::state);
        let previous = match flow {
            Flow::Output => std::mem::replace(&mut self.ctx.devices.speaker, state),
            Flow::Input => std::mem::replace(&mut self.ctx.devices.mic, state),
        };
        // Troca de dispositivo padrão também chega aqui: a lista do painel é relida.
        if let Some((_, kind)) = self.devices_panel_open() {
            let list_flow = if kind == PanelKind::Mic { Flow::Input } else { Flow::Output };
            if list_flow == flow {
                self.devices_list(flow);
            }
        }
        self.devices_redraw()?;
        // O aviso da ilha só vem de uma mudança de verdade, não da primeira leitura.
        if flow == Flow::Output && previous.is_some() && previous != state {
            self.island_volume_changed()?;
        }
        Ok(())
    }

    pub fn devices_brightness_changed(&mut self) -> Result<()> {
        self.ctx.devices.brightness = self.devices.brightness.as_ref().map(BrightnessService::monitors).unwrap_or_default();
        self.devices_redraw()
    }

    fn devices_list(&mut self, flow: Flow) {
        let (list, default) = audio_devices::devices(flow).unwrap_or_default();
        self.devices.devices = list;
        self.devices.default_device = default;
    }

    /// Nome amigável do monitor (o da barra, que vem do caminho do dispositivo).
    fn monitor_label(&self, device: &str, fallback: &str) -> String {
        self.bars
            .iter()
            .find(|b| b.monitor().device == device)
            .map(|b| format!("{}{}", b.monitor().identity.name, if b.monitor().primary { " (principal)" } else { "" }))
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Conteúdo dos painéis de som, microfone e brilho.
    /// Ao abrir o painel de som ou de microfone: lê a lista de dispositivos uma vez.
    pub fn devices_prepare(&mut self, kind: PanelKind) {
        if matches!(kind, PanelKind::Audio | PanelKind::Mic) {
            self.devices_list(if kind == PanelKind::Mic { Flow::Input } else { Flow::Output });
        }
    }

    pub fn devices_panel_list(&mut self, kind: PanelKind, scale: f32) -> DisplayList {
        let default_name = || self.devices.default_device.as_ref().and_then(|id| self.devices.devices.iter().find(|d| &d.id == id)).map(|d| d.name.clone());
        let items = || self.devices.devices.iter().map(|d| DeviceItem { id: d.id.clone(), name: d.name.clone(), current: self.devices.default_device.as_ref() == Some(&d.id) }).collect::<Vec<_>>();
        let tokens = &self.ctx.theme.tokens;
        let panel = match kind {
            PanelKind::Audio => {
                let v = self.ctx.devices.speaker;
                let muted = v.is_some_and(|v| v.muted);
                let p = v.map(|v| pct(v.level));
                let row = SliderRow { id: OUT.into(), label: default_name().unwrap_or_else(|| "Saída de som".into()), percent: p, muted: v.map(|v| v.muted), icon: speaker_icon(p.unwrap_or(0), muted) };
                device_panel("Som", &[row], &items(), scale, tokens)
            }
            PanelKind::Mic => {
                let v = self.ctx.devices.mic;
                let row = SliderRow { id: IN.into(), label: default_name().unwrap_or_else(|| "Microfone".into()), percent: v.map(|v| pct(v.level)), muted: v.map(|v| v.muted), icon: mic_icon(v.is_some_and(|v| v.muted)) };
                device_panel("Microfone", &[row], &items(), scale, tokens)
            }
            _ => {
                let rows: Vec<SliderRow> = self
                    .ctx
                    .devices
                    .brightness
                    .iter()
                    .map(|m| SliderRow { id: m.device.clone(), label: self.monitor_label(&m.device, &m.name), percent: m.percent, muted: None, icon: ICON_BRIGHTNESS })
                    .collect();
                device_panel("Brilho", &rows, &[], scale, tokens)
            }
        };
        let list = panel.list.clone();
        self.devices.panel = Some(panel);
        list
    }

    /// Ajusta o nível de uma linha (otimista: a barra mostra já, o sistema confirma depois).
    fn devices_set(&mut self, row: &str, percent: u32) -> Result<()> {
        let result = match row {
            OUT => {
                if let Some(v) = &mut self.ctx.devices.speaker {
                    v.level = percent as f32 / 100.0;
                }
                // O valor otimista já está no estado: o aviso do sistema não seria "mudança", então avisa aqui.
                self.island_volume_changed()?;
                audio_devices::set_volume(Flow::Output, percent)
            }
            IN => {
                if let Some(v) = &mut self.ctx.devices.mic {
                    v.level = percent as f32 / 100.0;
                }
                audio_devices::set_volume(Flow::Input, percent)
            }
            device => {
                if let Some(b) = &self.devices.brightness {
                    b.set(device, percent);
                    self.ctx.devices.brightness = b.monitors();
                }
                self.island_brightness_changed(percent)?;
                Ok(())
            }
        };
        if let Err(e) = result {
            tracing::warn!(row, "não consegui ajustar o nível: {e}");
        }
        self.devices_redraw()
    }

    /// Roda do mouse sobre um chip de som, microfone ou brilho. `true` se tratou.
    pub fn devices_wheel(&mut self, bar: usize, delta: i32) -> Result<bool> {
        let Some(chip) = self.bars[bar].hovered().map(str::to_string) else { return Ok(false) };
        match chip.as_str() {
            "audio" => {
                let cur = self.ctx.devices.speaker.map_or(0, |v| pct(v.level));
                self.devices_set(OUT, wheel(cur, delta, VOLUME_STEP))?;
            }
            "mic" => {
                let cur = self.ctx.devices.mic.map_or(0, |v| pct(v.level));
                self.devices_set(IN, wheel(cur, delta, VOLUME_STEP))?;
            }
            "brightness" => {
                let device = self.bars[bar].monitor().device.clone();
                let Some(cur) = self.ctx.devices.brightness_of(&device) else { return Ok(true) };
                self.devices_set(&device, wheel(cur, delta, BRIGHTNESS_STEP))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Botão pressionado no painel: começa a arrastar um slider (e já pula para o ponto).
    pub fn devices_down(&mut self, panel: usize, p: Point) -> Result<bool> {
        let Some((id, track)) = self.devices.panel.as_ref().and_then(|d| d.slider_at(p)) else { return Ok(false) };
        self.devices.drag = Some(id.clone());
        window::capture(self.panels[panel].hwnd());
        self.devices_set(&id, slider_percent(p.x, track.x, track.w))?;
        Ok(true)
    }

    pub fn devices_move(&mut self, p: Point) -> Result<bool> {
        let Some(id) = self.devices.drag.clone() else { return Ok(false) };
        let Some(track) = self.devices.panel.as_ref().and_then(|d| d.slider(&id)) else { return Ok(true) };
        let percent = slider_percent(p.x, track.x, track.w);
        let current = match id.as_str() {
            OUT => self.ctx.devices.speaker.map(|v| pct(v.level)),
            IN => self.ctx.devices.mic.map(|v| pct(v.level)),
            device => self.ctx.devices.brightness_of(device),
        };
        if current != Some(percent) {
            self.devices_set(&id, percent)?;
        }
        Ok(true)
    }

    /// Botão solto: termina o arraste ou trata mudo/dispositivo.
    pub fn devices_up(&mut self, p: Point) -> Result<bool> {
        if self.devices.drag.take().is_some() {
            window::release_capture();
            return Ok(true);
        }
        let Some(action) = self.devices.panel.as_ref().and_then(|d| d.hit(p)) else { return Ok(self.devices.panel.is_some()) };
        let result = match &action {
            DeviceAction::ToggleMute(row) => {
                let (flow, state) = if row == IN { (Flow::Input, self.ctx.devices.mic) } else { (Flow::Output, self.ctx.devices.speaker) };
                audio_devices::set_muted(flow, !state.is_some_and(|v| v.muted))
            }
            DeviceAction::Device(id) => audio_devices::set_default(id),
        };
        if let Err(e) = result {
            tracing::warn!(?action, "ação do painel falhou: {e}");
        }
        Ok(true)
    }

    /// Painel fechou: esquece desenho e arraste.
    pub fn devices_panel_closed(&mut self) {
        if self.devices.drag.take().is_some() {
            window::release_capture();
        }
        self.devices.panel = None;
    }
}
