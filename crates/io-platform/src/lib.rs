//! Платформенные адаптеры. В каркасе HID и системный ввод не открываются.

use io_core::application::{AppInfo, DeviceAccess};

pub fn application_info() -> AppInfo {
    AppInfo {
        name: "IO Type 84".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        device_access: DeviceAccess::NotImplemented,
    }
}
