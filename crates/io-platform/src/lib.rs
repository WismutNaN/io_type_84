//! Платформенные адаптеры. HID открывается только при явном подключении.

pub mod changes;
pub mod device;
mod input_ownership;
pub mod layout;
pub mod monitor;
pub mod protocol;
pub mod service;

use io_core::application::{AppInfo, DeviceAccess};

pub fn application_info() -> AppInfo {
    AppInfo {
        name: "IO Type 84".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        device_access: DeviceAccess::Available,
    }
}

mod action_runtime;
mod computer;

// Tags our synthetic output for diagnostics and future input-source loop prevention.
pub const INJECTED_INPUT_TAG: usize = 0x494f3834;
