//! One irreversible choice per physical press; output ownership belongs to the adapter.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HoldRepeat {
    pub delay_ms: u16,
    pub interval_ms: u16,
}

// Runtime identity, not a persisted/profile object. A resumed hold gets a new cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepeatSource {
    pub slot: u8,
    pub cycle: u64,
}

pub struct RepeatWindow {
    pub source: RepeatSource,
    pub remaining_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DepthChoice {
    pub id: String,
    pub slot: u8,
    pub light_um: u16,
    pub deep_um: u16,
    pub release_um: u16,
    pub light_action_id: String,
    pub deep_action_id: String,
    #[serde(default)]
    pub deep_repeat: Option<HoldRepeat>,
}

#[derive(Default)]
enum Phase {
    #[default]
    AwaitRelease,
    Ready,
    Pending,
    Committed,
}

#[derive(Default)]
pub(crate) struct ChoiceState {
    phase: Phase,
    last_sample: Option<u64>,
    repeat_at: Option<u64>,
    repeat_cycle: u64,
}

impl ChoiceState {
    fn arm_repeat(&mut self, rule: &DepthChoice, now: u64) {
        self.repeat_cycle = self.repeat_cycle.wrapping_add(1);
        self.repeat_at = rule
            .deep_repeat
            .as_ref()
            .map(|r| now.saturating_add(u64::from(r.delay_ms)));
    }

    pub fn repeat_window(&self, slot: u8, now: u64) -> Option<RepeatWindow> {
        let age = now.checked_sub(self.last_sample?)?;
        (matches!(self.phase, Phase::Committed) && self.repeat_at.is_some() && age < 100).then(
            || RepeatWindow {
                source: RepeatSource {
                    slot,
                    cycle: self.repeat_cycle,
                },
                remaining_ms: 100 - age,
            },
        )
    }

    pub fn expire(&mut self, now: u64) {
        if self
            .last_sample
            .is_some_and(|last| now.saturating_sub(last) >= 600 || now < last)
        {
            self.phase = Phase::AwaitRelease;
            self.last_sample = None;
            self.repeat_at = None;
        }
    }

    pub fn sample(&mut self, rule: &DepthChoice, depth: u16, now: u64) -> Option<String> {
        self.expire(now);
        if self
            .last_sample
            .is_some_and(|last| now.saturating_sub(last) >= 100)
        {
            self.repeat_at = None;
        }
        self.last_sample = Some(now);
        if depth <= rule.release_um {
            let output = matches!(self.phase, Phase::Pending).then(|| rule.light_action_id.clone());
            self.phase = Phase::Ready;
            self.repeat_at = None;
            return output;
        }
        if matches!(self.phase, Phase::AwaitRelease) {
            return None;
        }
        if matches!(self.phase, Phase::Committed) {
            let stop_depth = rule.deep_um.saturating_sub(200).max(rule.release_um + 100);
            if depth <= stop_depth {
                self.repeat_at = None;
            } else if depth >= rule.deep_um && self.repeat_at.is_none() {
                self.arm_repeat(rule, now);
            }
            return None;
        }
        if depth >= rule.deep_um {
            self.phase = Phase::Committed;
            self.arm_repeat(rule, now);
            return Some(rule.deep_action_id.clone());
        }
        if depth >= rule.light_um {
            self.phase = Phase::Pending;
        }
        None
    }

    pub fn tick(&mut self, rule: &DepthChoice, now: u64) -> Option<String> {
        self.expire(now);
        if self
            .last_sample
            .is_none_or(|last| now.saturating_sub(last) >= 100)
        {
            self.repeat_at = None;
        }
        if let (Some(repeat), Some(due)) = (&rule.deep_repeat, self.repeat_at)
            && matches!(self.phase, Phase::Committed)
            && now >= due
        {
            // Never catch up missed intervals with a burst of actions.
            self.repeat_at = Some(now.saturating_add(u64::from(repeat.interval_ms)));
            return Some(rule.deep_action_id.clone());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> DepthChoice {
        DepthChoice {
            id: "pgup".into(),
            slot: 105,
            light_um: 600,
            deep_um: 3000,
            release_um: 200,
            light_action_id: "page".into(),
            deep_action_id: "volume".into(),
            deep_repeat: None,
        }
    }
    fn run(samples: &[(u64, u16)]) -> Vec<String> {
        let mut state = ChoiceState::default();
        samples
            .iter()
            .filter_map(|&(time, depth)| state.sample(&rule(), depth, time))
            .collect()
    }
    #[test]
    fn light_is_delayed_until_release_and_noise_does_not_trigger() {
        assert_eq!(
            run(&[
                (0, 0),
                (1, 180),
                (2, 0),
                (3, 800),
                (4, 500),
                (5, 800),
                (6, 200),
                (7, 0)
            ]),
            ["page"]
        );
        assert!(run(&[(0, 0), (1, 800), (2, 900)]).is_empty());
    }
    #[test]
    fn deep_commits_once_and_cancels_light_through_every_return_crossing() {
        assert_eq!(
            run(&[
                (0, 0),
                (1, 800),
                (2, 3000),
                (3, 3980),
                (4, 2800),
                (5, 3100),
                (6, 700),
                (7, 0),
                (8, 800),
                (9, 0)
            ]),
            ["volume", "page"]
        );
        assert_eq!(run(&[(0, 0), (1, 3980), (2, 0)]), ["volume"]);
    }
    #[test]
    fn startup_and_lost_stream_cancel_pending_until_measured_release() {
        assert!(run(&[(0, 800), (1, 3980), (2, 0)]).is_empty());
        assert!(run(&[(0, 0), (1, 800), (602, 0)]).is_empty());
        assert_eq!(
            run(&[
                (0, 0),
                (1, 800),
                (602, 3900),
                (603, 0),
                (604, 800),
                (605, 0)
            ]),
            ["page"]
        );
    }

    #[test]
    fn repeat_delays_then_ticks_without_light_action_or_catch_up_bursts() {
        let mut r = rule();
        r.deep_repeat = Some(HoldRepeat {
            delay_ms: 350,
            interval_ms: 80,
        });
        let mut s = ChoiceState::default();
        s.sample(&r, 0, 0);
        assert_eq!(s.sample(&r, 3100, 10), Some("volume".into()));
        for time in (50..=350).step_by(50) {
            assert_eq!(s.sample(&r, 3100, time), None);
            assert_eq!(s.tick(&r, time), None);
        }
        assert_eq!(s.tick(&r, 360), Some("volume".into()));
        s.sample(&r, 3000, 420);
        assert_eq!(s.tick(&r, 439), None);
        assert_eq!(s.tick(&r, 440), Some("volume".into()));
        // Host timer stalls while samples are still flowing: no catch-up loop.
        for time in (450..=650).step_by(50) {
            s.sample(&r, 3000, time);
        }
        assert_eq!(s.tick(&r, 650), Some("volume".into()));
        assert_eq!(s.tick(&r, 651), None);
        s.sample(&r, 2790, 660);
        assert_eq!(s.tick(&r, 680), None);
        assert_eq!(s.sample(&r, 0, 690), None);
        assert_eq!(s.tick(&r, 750), None);
        s.sample(&r, 800, 800);
        assert_eq!(s.sample(&r, 0, 810), Some("page".into()));
    }

    #[test]
    fn repeat_stops_on_stale_input_and_resumes_only_after_a_fresh_delay() {
        let mut r = rule();
        r.deep_repeat = Some(HoldRepeat {
            delay_ms: 100,
            interval_ms: 50,
        });
        let mut s = ChoiceState::default();
        s.sample(&r, 0, 0);
        s.sample(&r, 3500, 1);
        s.sample(&r, 3500, 90);
        assert!(s.tick(&r, 101).is_some());
        assert!(s.tick(&r, 190).is_none()); // input TTL is 100 ms, not 600 ms
        s.sample(&r, 3500, 200);
        assert!(s.tick(&r, 201).is_none());
        s.sample(&r, 3500, 280);
        assert!(s.tick(&r, 300).is_some());
        assert!(s.tick(&r, 1000).is_none());
        s.sample(&r, 3500, 1001);
        assert!(s.tick(&r, 1050).is_none()); // full stale timeout requires release
        assert!(s.sample(&r, 0, 1051).is_none());
    }
}
