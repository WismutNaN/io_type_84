//! Каталог действий и жесты независимы от устройства, Win32 и хранения профилей.
use crate::{
    depth::ComputerAction,
    keyboard::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActionCommand {
    Media { action: ComputerAction },
    Key { key: u8, modifiers: u8 },
    Text { text: String },
    Application { application: ApplicationId },
    Macro { steps: Vec<ActionStep> },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApplicationId {
    Word,
    Notepad,
    Calculator,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActionStep {
    Key { key: u8, modifiers: u8 },
    Text { text: String },
    Delay { ms: u16 },
    Media { action: ComputerAction },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionDefinition {
    pub id: String,
    pub name: String,
    pub command: ActionCommand,
    #[serde(default)]
    pub platform_commands: PlatformCommands,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCommands {
    pub windows: Option<ActionCommand>,
    pub linux: Option<ActionCommand>,
    pub macos: Option<ActionCommand>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GestureRule {
    pub id: String,
    pub slots: Vec<u8>,
    pub threshold_um: u16,
    pub release_um: u16,
    pub hold_ms: u16,
    pub action_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationProfile {
    pub actions: Vec<ActionDefinition>,
    pub gestures: Vec<GestureRule>,
}
pub fn supported_key(key: u8) -> bool {
    matches!(key, 4..=69 | 73..=82 | 224..=231)
}
fn valid_text(text: &str) -> bool {
    text.chars().count() <= 2000 && !text.contains('\0')
}
impl AutomationProfile {
    pub fn validate(&self) -> Result<()> {
        let fail = || {
            AppError::new(
                "invalidAutomation",
                "Проверьте каталог действий и условия жестов.",
            )
        };
        let valid_id = |s: &str| {
            !s.is_empty()
                && s.len() <= 80
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        };
        if self.actions.len() > 256 || self.gestures.len() > 256 {
            return Err(fail());
        }
        let mut ids = BTreeSet::new();
        for a in &self.actions {
            if !valid_id(&a.id)
                || !ids.insert(&a.id)
                || a.name.trim().is_empty()
                || a.name.chars().count() > 100
            {
                return Err(fail());
            }
            let valid = match &a.command {
                ActionCommand::Media { .. } | ActionCommand::Application { .. } => true,
                ActionCommand::Key { key, .. } => supported_key(*key),
                ActionCommand::Text { text } => valid_text(text),
                ActionCommand::Macro { steps } => {
                    !steps.is_empty()
                        && steps.len() <= 128
                        && steps
                            .iter()
                            .map(|s| {
                                if let ActionStep::Delay { ms } = s {
                                    u32::from(*ms)
                                } else {
                                    0
                                }
                            })
                            .sum::<u32>()
                            <= 30000
                        && steps.iter().all(|s| match s {
                            ActionStep::Key { key, .. } => supported_key(*key),
                            ActionStep::Text { text } => valid_text(text),
                            ActionStep::Delay { ms } => *ms <= 10000,
                            ActionStep::Media { .. } => true,
                        })
                        && steps
                            .iter()
                            .map(|s| {
                                if let ActionStep::Text { text } = s {
                                    text.chars().count()
                                } else {
                                    0
                                }
                            })
                            .sum::<usize>()
                            <= 4000
                }
            };
            if !valid {
                return Err(fail());
            }
            for command in [
                &a.platform_commands.windows,
                &a.platform_commands.linux,
                &a.platform_commands.macos,
            ]
            .into_iter()
            .flatten()
            {
                // Та же валидация для каждой платформы, без вложенных переопределений.
                AutomationProfile {
                    actions: vec![ActionDefinition {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        command: command.clone(),
                        platform_commands: PlatformCommands::default(),
                    }],
                    gestures: vec![],
                }
                .validate()?;
            }
        }
        let mut rule_ids = BTreeSet::new();
        for r in &self.gestures {
            if !valid_id(&r.id)
                || !rule_ids.insert(&r.id)
                || !ids.contains(&r.action_id)
                || r.slots.is_empty()
                || r.slots.len() > 8
                || r.slots.iter().any(|s| *s >= 128)
                || r.slots.iter().collect::<BTreeSet<_>>().len() != r.slots.len()
                || !(300..=3200).contains(&r.threshold_um)
                || u32::from(r.release_um) + 100 > u32::from(r.threshold_um)
                || r.hold_ms > 10000
            {
                return Err(fail());
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct GestureState {
    armed: bool,
    since: Option<u64>,
}
pub struct GestureEngine {
    pub profile: AutomationProfile,
    samples: [Option<(u16, u64)>; 128],
    states: Vec<GestureState>,
}
impl GestureEngine {
    pub fn new(profile: AutomationProfile) -> Self {
        let states = (0..profile.gestures.len())
            .map(|_| GestureState::default())
            .collect();
        Self {
            profile,
            samples: [None; 128],
            states,
        }
    }
    pub fn sample(&mut self, slot: u8, depth: u16, now: u64) {
        if let Some(sample) = self.samples.get_mut(usize::from(slot)) {
            *sample = Some((depth, now));
        }
    }
    pub fn tick(&mut self, now: u64) -> Vec<String> {
        let mut fired = Vec::new();
        for (rule, state) in self.profile.gestures.iter().zip(&mut self.states) {
            // Каждый член сочетания должен быть подтверждён свежим аналоговым пакетом.
            let samples: Option<Vec<_>> = rule
                .slots
                .iter()
                .map(|s| {
                    self.samples[usize::from(*s)]
                        .filter(|(_, time)| now.saturating_sub(*time) < 600)
                })
                .collect();
            let Some(samples) = samples else {
                *state = GestureState::default();
                continue;
            };
            if samples.iter().any(|(depth, _)| *depth <= rule.release_um) {
                state.armed = true;
                state.since = None;
            }
            if !samples.iter().all(|(depth, _)| *depth >= rule.threshold_um) {
                state.since = None;
                continue;
            }
            if state.armed {
                let since = *state.since.get_or_insert(now);
                if now.saturating_sub(since) >= u64::from(rule.hold_ms) {
                    fired.push(rule.action_id.clone());
                    state.armed = false;
                    state.since = None;
                }
            }
        }
        fired
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> AutomationProfile {
        AutomationProfile {
            actions: vec![ActionDefinition {
                id: "word".into(),
                name: "Word".into(),
                command: ActionCommand::Application {
                    application: ApplicationId::Word,
                },
                platform_commands: PlatformCommands::default(),
            }],
            gestures: vec![GestureRule {
                id: "hold".into(),
                slots: vec![49, 50],
                threshold_um: 1200,
                release_um: 600,
                hold_ms: 800,
                action_id: "word".into(),
            }],
        }
    }
    #[test]
    fn chord_hold_requires_release_continuity_and_fires_once() {
        let mut engine = GestureEngine::new(profile());
        for slot in [49, 50] {
            engine.sample(slot, 2000, 0);
        }
        assert!(engine.tick(0).is_empty());
        engine.sample(49, 0, 10);
        assert!(engine.tick(10).is_empty());
        for time in [20, 400, 800] {
            for slot in [49, 50] {
                engine.sample(slot, 2000, time);
            }
            assert!(engine.tick(time).is_empty());
        }
        assert_eq!(engine.tick(820), vec!["word"]);
        assert!(engine.tick(900).is_empty());
        engine.sample(49, 0, 950);
        engine.tick(950);
        engine.sample(49, 2000, 1000);
        engine.tick(1000);
        assert!(engine.tick(1900).is_empty()); // потеря пакетов не является удержанием
        for slot in [49, 50] {
            engine.sample(slot, 2000, 2000);
        }
        assert!(engine.tick(2000).is_empty());
    }
    #[test]
    fn references_and_unbounded_actions_are_rejected() {
        let mut p = profile();
        assert!(p.validate().is_ok());
        p.gestures[0].action_id = "missing".into();
        assert!(p.validate().is_err());
        p = profile();
        p.gestures[0].slots = vec![49, 49];
        assert!(p.validate().is_err());
        p = profile();
        p.actions[0].command = ActionCommand::Macro {
            steps: vec![ActionStep::Delay { ms: 10000 }; 4],
        };
        assert!(p.validate().is_err());
    }
}
