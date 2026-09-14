//! One irreversible choice per physical press; output ownership belongs to the adapter.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
}

impl ChoiceState {
    pub fn expire(&mut self, now: u64) {
        if self
            .last_sample
            .is_some_and(|last| now.saturating_sub(last) >= 600 || now < last)
        {
            self.phase = Phase::AwaitRelease;
            self.last_sample = None;
        }
    }

    pub fn sample(&mut self, rule: &DepthChoice, depth: u16, now: u64) -> Option<String> {
        self.expire(now);
        self.last_sample = Some(now);
        if depth <= rule.release_um {
            let output = matches!(self.phase, Phase::Pending).then(|| rule.light_action_id.clone());
            self.phase = Phase::Ready;
            return output;
        }
        if matches!(self.phase, Phase::AwaitRelease | Phase::Committed) {
            return None;
        }
        if depth >= rule.deep_um {
            self.phase = Phase::Committed;
            return Some(rule.deep_action_id.clone());
        }
        if depth >= rule.light_um {
            self.phase = Phase::Pending;
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
}
