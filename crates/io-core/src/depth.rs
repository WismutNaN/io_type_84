//! Детерминированные условия глубины. Время передаёт адаптер, ОС здесь отсутствует.
use crate::keyboard::{AppError, Result};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComputerAction {
    VolumeUp,
    VolumeDown,
    Mute,
    PlayPause,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DepthRule {
    pub slot: u8,
    pub threshold_um: u16,
    pub release_um: u16,
    pub action: ComputerAction,
}

impl DepthRule {
    pub fn validate(&self) -> Result<()> {
        if self.slot >= 128
            || !(300..=3200).contains(&self.threshold_um)
            || u32::from(self.release_um) + 100 > u32::from(self.threshold_um)
            || self.release_um > 3100
        {
            return Err(AppError::new(
                "invalidRule",
                "Порог должен быть 0,30–3,20 мм, возврат — минимум на 0,10 мм выше.",
            ));
        }
        Ok(())
    }
}

pub struct DepthTrigger {
    pub rule: DepthRule,
    armed: bool,
    last_ms: Option<u64>,
}
impl DepthTrigger {
    pub fn new(rule: DepthRule) -> Self {
        Self {
            rule,
            armed: false,
            last_ms: None,
        }
    }
    pub fn sample(&mut self, depth: u16, now_ms: u64) -> Option<ComputerAction> {
        if self
            .last_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= 600)
        {
            self.armed = false;
        }
        self.last_ms = Some(now_ms);
        // После старта/пропажи сигнала сначала требуется настоящий возврат ниже порога.
        if depth <= self.rule.release_um {
            self.armed = true;
        }
        if depth >= self.rule.threshold_um && self.armed {
            self.armed = false;
            return Some(self.rule.action);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_action_per_crossing_and_no_action_on_resume_held() {
        let mut trigger = DepthTrigger::new(DepthRule {
            slot: 31,
            threshold_um: 2400,
            release_um: 1800,
            action: ComputerAction::VolumeUp,
        });
        assert_eq!(trigger.sample(3000, 0), None);
        assert_eq!(trigger.sample(0, 10), None);
        assert_eq!(trigger.sample(2500, 20), Some(ComputerAction::VolumeUp));
        for (time, depth) in [(30, 3000), (40, 2350), (50, 2500), (1000, 3000)] {
            assert_eq!(trigger.sample(depth, time), None);
        }
        assert_eq!(trigger.sample(1700, 1010), None);
        assert_eq!(trigger.sample(2500, 1020), Some(ComputerAction::VolumeUp));
    }
    #[test]
    fn invalid_thresholds_are_rejected() {
        let rule = DepthRule {
            slot: 31,
            threshold_um: 100,
            release_um: 100,
            action: ComputerAction::VolumeDown,
        };
        assert!(rule.validate().is_err());
    }
}
