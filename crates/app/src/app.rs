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
    /// Ponto no chip do chat: o Claude está respondendo ou há resposta não lida.
    pub chat_badge: bool,
}

/// O que os chips de uso mostram: as contas do Claude, do Codex e do Grok no Windows e no WSL.
#[derive(Debug, Clone, Default)]
pub struct UsageState {
    /// As contas do Claude e quando terminou a última consulta.
    pub claude: Option<zefiro_integration_usage::claude::ClaudeSnapshot>,
    pub codex: Option<zefiro_integration_usage::codex::CodexSnapshot>,
    pub grok: Option<zefiro_integration_usage::grok::GrokSnapshot>,
    /// O número de cada chip na última vez que o dado chegou: a barra só é refeita se ele mudou.
    /// Comparar com o snapshot antigo recalculado agora não serve: depois do `resets_at`, ele já dá
    /// 0 % também, e o chip ficaria com o número velho.
    claude_shown: Option<f32>,
    codex_shown: Option<f32>,
    grok_shown: Option<f32>,
}

impl UsageState {
    /// % do chip do Claude: a maior janela de 5 h entre as contas (o último valor bom numa falha
    /// passageira).
    pub fn claude_chip(&self) -> Option<f32> {
        self.claude.as_ref().and_then(|s| zefiro_integration_usage::claude::chip_percent_accounts(&s.accounts, crate::usage_loop::now_unix()))
    }

    /// Guarda as contas do Claude; `true` se o número do chip mudou.
    pub fn set_claude(&mut self, snapshot: Option<zefiro_integration_usage::claude::ClaudeSnapshot>, now_unix: i64) -> bool {
        self.claude = snapshot;
        let shown = self.claude.as_ref().and_then(|s| zefiro_integration_usage::claude::chip_percent_accounts(&s.accounts, now_unix));
        std::mem::replace(&mut self.claude_shown, shown) != shown
    }

    /// Guarda as contas do Codex; `true` se o número do chip mudou.
    pub fn set_codex(&mut self, snapshot: Option<zefiro_integration_usage::codex::CodexSnapshot>, now_unix: i64) -> bool {
        self.codex = snapshot;
        let shown = self.codex.as_ref().and_then(|s| zefiro_integration_usage::codex::codex_chip_percent(&s.accounts, now_unix));
        std::mem::replace(&mut self.codex_shown, shown) != shown
    }

    /// Guarda as contas do Grok; `true` se o número do chip mudou.
    pub fn set_grok(&mut self, snapshot: Option<zefiro_integration_usage::grok::GrokSnapshot>, now_unix: i64) -> bool {
        self.grok = snapshot;
        let shown = self.grok.as_ref().and_then(|s| zefiro_integration_usage::grok::grok_chip_percent(&s.accounts, now_unix));
        std::mem::replace(&mut self.grok_shown, shown) != shown
    }

    /// % do chip do Grok: a maior cota semanal entre as contas.
    pub fn grok_chip(&self) -> Option<f32> {
        self.grok.as_ref().and_then(|s| zefiro_integration_usage::grok::grok_chip_percent(&s.accounts, crate::usage_loop::now_unix()))
    }

    /// Os slides do chip `ia`: um por conta e origem (a mesma conta no Windows e no WSL vira dois
    /// slides com o mesmo número, para escolher qual aparece), arrumados pelo `[ia]`. Uma IA sem
    /// resposta ainda não tem slide.
    pub fn ai_slides(&self, ai: &zefiro_core::config::AiConfig, now_unix: i64) -> zefiro_bar::ai::Arranged {
        use zefiro_bar::ai::{Slide, account_slide_key, arrange, origin_tag};
        use zefiro_integration_chat::agent::Agent;
        use zefiro_integration_usage::origins::{Account, many_distros, origins_label};
        // Cada slide com o nome da conta (plano ou e-mail), para desempatar duas na mesma origem.
        fn slides<T>(agent: Agent, a: &Account<T>, many: bool, percent: Option<f32>, account: Option<&str>) -> Vec<(Slide, Option<String>)> {
            a.origins
                .iter()
                .map(|o| {
                    let one = std::slice::from_ref(o);
                    (Slide { key: account_slide_key(agent, o, &a.key), agent, origin: origins_label(one, many), tag: origin_tag(one), percent }, account.map(String::from))
                })
                .collect()
        }
        fn capitalized(s: &str) -> String {
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
        let mut all = Vec::new();
        if let Some(s) = &self.claude {
            let many = many_distros(&s.accounts);
            all.extend(s.accounts.iter().flat_map(|a| slides(Agent::Claude, a, many, zefiro_integration_usage::claude::chip_percent_accounts(std::slice::from_ref(a), now_unix), a.data.email.as_deref().or(a.data.plan.as_deref()))));
        }
        if let Some(s) = &self.codex {
            let many = many_distros(&s.accounts);
            all.extend(s.accounts.iter().flat_map(|a| slides(Agent::Codex, a, many, zefiro_integration_usage::codex::codex_chip_percent(std::slice::from_ref(a), now_unix), a.data.usage.plan.as_deref().map(capitalized).as_deref())));
        }
        if let Some(s) = &self.grok {
            let many = many_distros(&s.accounts);
            all.extend(s.accounts.iter().flat_map(|a| slides(Agent::Grok, a, many, zefiro_integration_usage::grok::grok_chip_percent(std::slice::from_ref(a), now_unix), a.data.email.as_deref().or(a.data.plan.as_deref()))));
        }
        // Duas contas da mesma IA na mesma origem: o nome ganha o plano ou o e-mail.
        let names: Vec<(Agent, String)> = all.iter().map(|(s, _)| (s.agent, s.origin.clone())).collect();
        let all = all
            .into_iter()
            .map(|(mut s, account)| {
                if names.iter().filter(|n| n.0 == s.agent && n.1 == s.origin).count() > 1
                    && let Some(account) = account
                {
                    s.origin = format!("{} · {account}", s.origin);
                }
                s
            })
            .collect();
        arrange(all, ai)
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

#[cfg(test)]
mod tests {
    use super::*;
    use zefiro_integration_usage::codex::{CodexEntry, CodexSnapshot, CodexUsage, Window};
    use zefiro_integration_usage::origins::{Account, OriginKind};

    #[test]
    fn ai_slides_have_one_account_each_with_its_origin() {
        use zefiro_integration_usage::grok::{GrokEntry, GrokSnapshot, GrokUsage};
        let t = 1_792_000_000;
        let wsl = OriginKind::Wsl("Ubuntu-24.04".into());
        let codex = |origins: Vec<OriginKind>, p: f32| Account { key: format!("{origins:?}"), origins, stopped: false, data: CodexEntry { modified: std::time::SystemTime::UNIX_EPOCH, usage: CodexUsage { primary: Some(Window { used_percent: p, window_minutes: 300, resets_at: t + 60 }), ..Default::default() } } };
        let grok = GrokEntry { result: Some(Ok(GrokUsage { percent: Some(25.0), period_end: Some(t + 60), ..Default::default() })), ..Default::default() };
        let state = UsageState {
            codex: Some(CodexSnapshot { accounts: vec![codex(vec![OriginKind::Windows], 1.0), codex(vec![wsl.clone()], 7.0)], wsl_stopped: false }),
            grok: Some(GrokSnapshot { accounts: vec![Account { key: "p1".into(), origins: vec![wsl.clone()], stopped: false, data: grok }], ..Default::default() }),
            ..Default::default()
        };
        let a = state.ai_slides(&zefiro_core::config::AiConfig::default(), t);
        let shown: Vec<(String, &str, Option<f32>)> = a.visible.iter().map(|s| (s.name(), s.tag, s.percent)).collect();
        assert_eq!(shown, [("Codex · Windows".to_string(), "Win", Some(1.0)), ("Codex · WSL".to_string(), "WSL", Some(7.0)), ("Grok · WSL".to_string(), "WSL", Some(25.0))], "sem o Claude (ainda sem resposta)");
        let codex_windows = zefiro_bar::ai::account_slide_key(zefiro_integration_chat::agent::Agent::Codex, &OriginKind::Windows, &format!("{:?}", vec![OriginKind::Windows]));
        let hidden = zefiro_core::config::AiConfig { hidden: vec![codex_windows.clone()], ..Default::default() };
        let a = state.ai_slides(&hidden, t);
        assert_eq!((a.visible.len(), a.hidden[0].key.as_str()), (2, codex_windows.as_str()));
        // A mesma conta no Windows e no WSL: um slide por origem, para escolher qual aparece.
        let merged = UsageState { codex: Some(CodexSnapshot { accounts: vec![codex(vec![OriginKind::Windows, wsl.clone()], 1.0)], wsl_stopped: false }), ..Default::default() };
        let a = merged.ai_slides(&zefiro_core::config::AiConfig::default(), t);
        let shown: Vec<(String, &str)> = a.visible.iter().map(|s| (s.name(), s.tag)).collect();
        assert_eq!(shown, [("Codex · Windows".to_string(), "Win"), ("Codex · WSL".to_string(), "WSL")]);
        assert!(a.visible[0].key.starts_with("codex:windows:") && a.visible[1].key.starts_with("codex:Ubuntu-24.04:"));
        // Duas contas do Codex no mesmo WSL (o usuário entrou com outra): dois slides, cada um com a
        // sua chave, e o nome diz o plano.
        let mut other = codex(vec![wsl.clone()], 9.0);
        other.key = "outra".into();
        other.data.usage.plan = Some("plus".into());
        let mut mine = codex(vec![wsl.clone()], 1.0);
        mine.data.usage.plan = Some("go".into());
        let two = UsageState { codex: Some(CodexSnapshot { accounts: vec![mine, other], wsl_stopped: false }), ..Default::default() };
        let a = two.ai_slides(&zefiro_core::config::AiConfig::default(), t);
        assert_ne!(a.visible[0].key, a.visible[1].key);
        let names: Vec<(String, Option<f32>)> = a.visible.iter().map(|s| (s.name(), s.percent)).collect();
        assert_eq!(names, [("Codex · WSL · Go".to_string(), Some(1.0)), ("Codex · WSL · Plus".to_string(), Some(9.0))]);
    }

    #[test]
    fn codex_chip_redraws_when_window_resets() {
        let t = 1_792_000_000;
        let usage = CodexUsage { primary: Some(Window { used_percent: 80.0, window_minutes: 300, resets_at: t }), ..Default::default() };
        let account = Account { key: "c1".into(), origins: vec![OriginKind::Windows], stopped: false, data: CodexEntry { modified: std::time::SystemTime::UNIX_EPOCH, usage } };
        let snapshot = CodexSnapshot { accounts: vec![account], wsl_stopped: false };
        let mut state = UsageState::default();
        assert!(state.set_codex(Some(snapshot.clone()), t - 10), "1º valor");
        assert!(!state.set_codex(Some(snapshot.clone()), t - 5), "mesmo número");
        // O mesmo snapshot depois do `resets_at`: o chip desenhado (80 %) tem que virar 0 %.
        assert!(state.set_codex(Some(snapshot), t + 1), "janela reiniciou");
    }
}
