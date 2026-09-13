//! Контракты редактора и наблюдения. Единицы хода — целые микрометры.

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AppError {}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub firmware: String,
    pub frame_version: u8,
    pub rt_precision: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BindingRecord {
    pub page: u8,
    pub parameters: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Actuation {
    pub trigger_um: u16,
    pub press_um: u16,
    pub release_um: u16,
    pub rapid_trigger: bool,
    pub whole_travel: bool,
    pub rampage: bool,
    pub axis_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KeyConfiguration {
    pub slot: u8,
    pub base: BindingRecord,
    pub function: BindingRecord,
    pub actuation: Actuation,
    pub led_id: u8,
    pub color: Rgb,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LightingSettings {
    pub mode: u8,
    pub color: Rgb,
    pub secondary_color: Rgb,
    pub color_mode: u8,
    pub brightness: u8,
    pub speed: u8,
    pub direction: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    pub report_rate: u8,
    pub top_dead_zone_um: u16,
    pub bottom_dead_zone_um: u16,
    pub key_delay: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacroStep {
    pub key_code: u8,
    pub pressed: bool,
    pub delay_ms: u16,
    pub kind: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HardwareMacro {
    pub id: u8,
    pub steps: Vec<MacroStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DksConfiguration {
    pub index: u8,
    pub thresholds: [u8; 4],
    pub actions: [u8; 4],
    pub states: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardSnapshot {
    pub revision: String,
    pub identity: DeviceIdentity,
    pub keys: Vec<KeyConfiguration>,
    pub lighting: LightingSettings,
    pub performance: PerformanceSettings,
    pub macros: Vec<HardwareMacro>,
    pub dks: Vec<DksConfiguration>,
    pub macro_bytes_used: u32,
    pub macro_write_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Edit {
    Binding {
        slots: Vec<u8>,
        function_layer: bool,
        binding: BindingRecord,
    },
    Actuation {
        slots: Vec<u8>,
        value: Actuation,
    },
    Lighting {
        value: LightingSettings,
    },
    Color {
        slots: Vec<u8>,
        color: Rgb,
    },
    Performance {
        value: PerformanceSettings,
    },
    Macros {
        values: Vec<HardwareMacro>,
    },
    Dks {
        value: DksConfiguration,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequest {
    pub base_revision: String,
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub block: String,
    pub changed_bytes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangePreview {
    pub token: String,
    pub changes: Vec<ChangeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub snapshot: KeyboardSnapshot,
    pub recovery_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KeyTravel {
    pub slot: u8,
    pub travel_um: u16,
    pub max_travel_um: u16,
    pub adc: u16,
    pub age_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KeyPress {
    pub sequence: u32,
    pub slot: u8,
    pub peak_um: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LiveColor {
    pub led_id: u8,
    pub color: Rgb,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonitorFrame {
    pub active: bool,
    pub travel: Vec<KeyTravel>,
    pub history: Vec<KeyPress>,
    pub colors: Vec<LiveColor>,
    pub color_age_ms: Option<u32>,
    pub packets: u32,
    pub message: Option<String>,
    pub rules_enabled: bool,
    pub rule_firings: u32,
    pub rule_error: Option<String>,
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        crate::depth::ComputerAction::decl(config),
        crate::depth::DepthRule::decl(config),
        AppError::decl(config),
        DeviceIdentity::decl(config),
        Rgb::decl(config),
        BindingRecord::decl(config),
        Actuation::decl(config),
        KeyConfiguration::decl(config),
        LightingSettings::decl(config),
        PerformanceSettings::decl(config),
        MacroStep::decl(config),
        HardwareMacro::decl(config),
        DksConfiguration::decl(config),
        KeyboardSnapshot::decl(config),
        Edit::decl(config),
        ChangeRequest::decl(config),
        ChangeSummary::decl(config),
        ChangePreview::decl(config),
        ApplyResult::decl(config),
        KeyTravel::decl(config),
        KeyPress::decl(config),
        LiveColor::decl(config),
        MonitorFrame::decl(config),
    ]
    .into_iter()
    .map(|s| format!("export {s}\n\n"))
    .collect()
}
