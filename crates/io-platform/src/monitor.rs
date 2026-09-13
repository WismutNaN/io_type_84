//! Ограниченная история физических нажатий; никаких файлов и глобальных hooks.
use io_core::keyboard::*;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const SAMPLE_TTL: Duration = Duration::from_millis(600);

pub struct MonitorState {
    pub active: bool,
    pub message: Option<String>,
    pub packets: u32,
    travel: BTreeMap<u8, (KeyTravel, Instant)>,
    history: Vec<KeyPress>,
    down: [bool; 128],
    sequence: u32,
    colors: Vec<LiveColor>,
    color_time: Option<Instant>,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            active: false,
            message: None,
            packets: 0,
            travel: BTreeMap::new(),
            history: Vec::new(),
            down: [false; 128],
            sequence: 0,
            colors: Vec::new(),
            color_time: None,
        }
    }
}

impl MonitorState {
    pub fn pause(&mut self) {
        self.active = false;
        self.down.fill(false);
        self.travel.clear();
        self.colors.clear();
        self.color_time = None;
    }
    pub fn ingest(&mut self, packet: &[u8]) {
        self.ingest_at(packet, Instant::now());
    }
    fn ingest_at(&mut self, packet: &[u8], now: Instant) {
        if packet.len() < 14 || packet[..2] != [0x55, 0xfb] || packet[2] >= 128 {
            return;
        }
        let travel = u16::from_le_bytes([packet[10], packet[11]]);
        let max = u16::from_le_bytes([packet[12], packet[13]]);
        if travel > 1000 || max > 1000 {
            return;
        }
        let slot = packet[2];
        let depth = travel * 10;
        // Потерянный release не должен склеивать два отдельных движения.
        if self
            .travel
            .get(&slot)
            .is_some_and(|(_, time)| now.duration_since(*time) >= SAMPLE_TTL)
        {
            self.down[usize::from(slot)] = false;
        }
        self.packets = self.packets.saturating_add(1);
        self.travel.insert(
            slot,
            (
                KeyTravel {
                    slot,
                    travel_um: depth,
                    max_travel_um: max * 10,
                    adc: u16::from_le_bytes([packet[8], packet[9]]),
                    age_ms: 0,
                },
                now,
            ),
        );
        // История начала физического движения; не утверждение о HID key-down/RT.
        if depth >= 300 && !self.down[usize::from(slot)] {
            self.down[usize::from(slot)] = true;
            self.sequence = self.sequence.wrapping_add(1);
            self.history.insert(
                0,
                KeyPress {
                    sequence: self.sequence,
                    slot,
                    peak_um: depth,
                },
            );
            self.history.truncate(20);
        } else if self.down[usize::from(slot)] {
            if let Some(event) = self.history.iter_mut().find(|e| e.slot == slot) {
                event.peak_um = event.peak_um.max(depth);
            }
            if depth <= 150 {
                self.down[usize::from(slot)] = false;
            }
        }
    }
    pub fn set_colors(&mut self, colors: Vec<LiveColor>) {
        self.colors = colors;
        self.color_time = Some(Instant::now());
    }
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
    pub fn frame(&self) -> MonitorFrame {
        MonitorFrame {
            active: self.active,
            travel: self
                .travel
                .values()
                .map(|(value, time)| {
                    let mut v = value.clone();
                    v.age_ms = time.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
                    v
                })
                .collect(),
            history: self.history.clone(),
            colors: self.colors.clone(),
            color_age_ms: self
                .color_time
                .map(|t| t.elapsed().as_millis().min(u128::from(u32::MAX)) as u32),
            packets: self.packets,
            message: self.message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(slot: u8, um: u16) -> [u8; 14] {
        let mut p = [0; 14];
        p[..3].copy_from_slice(&[0x55, 0xfb, slot]);
        p[10..12].copy_from_slice(&(um / 10).to_le_bytes());
        p[12..14].copy_from_slice(&400u16.to_le_bytes());
        p
    }
    #[test]
    fn history_is_bounded_and_repeat_reports_do_not_duplicate_presses() {
        let mut state = MonitorState::default();
        for _ in 0..25 {
            state.ingest(&packet(49, 300));
            state.ingest(&packet(49, 2230));
            state.ingest(&packet(49, 0));
        }
        let frame = state.frame();
        assert_eq!(frame.history.len(), 20);
        assert_eq!(frame.history[0].sequence, 25);
        assert_eq!(frame.history[0].peak_um, 2230);
        assert_eq!(frame.travel[0].travel_um, 0);
        state.ingest(&[0x55, 0xfb]);
        state.ingest(&packet(255, 100));
        assert_eq!(state.frame().history.len(), 20);
        state.ingest(&packet(49, 300));
        state.pause();
        assert!(state.frame().travel.is_empty());
        state.ingest(&packet(49, 300));
        assert_eq!(state.frame().history[0].sequence, 27);
    }
    #[test]
    fn missing_release_rearms_history_but_fresh_hold_does_not() {
        let mut state = MonitorState::default();
        let now = Instant::now();
        state.ingest_at(&packet(49, 900), now);
        state.ingest_at(&packet(49, 1000), now + Duration::from_millis(50));
        assert_eq!(state.history.len(), 1);
        state.ingest_at(&packet(49, 1200), now + Duration::from_secs(1));
        assert_eq!(state.history.len(), 2);
    }
    #[test]
    fn idle_noise_does_not_fill_the_history() {
        let mut state = MonitorState::default();
        for depth in [0, 100, 180, 160, 0] {
            state.ingest(&packet(58, depth));
        }
        assert!(state.history.is_empty());
    }
}
