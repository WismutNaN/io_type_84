use serde::Serialize;
use ts_rs::{Config, TS};

/// Состояние реализации транспорта, а не результат обнаружения клавиатуры.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DeviceAccess {
    NotImplemented,
    Available,
}

/// Ответ проверки связи между оболочкой и Rust.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub device_access: DeviceAccess,
}

/// Единственный источник TypeScript-типов для существующего IPC.
pub fn typescript_contracts() -> String {
    let config = Config::default();
    let mut declarations = format!(
        "// Сгенерировано из io-core. Обновить: npm run contracts\n\nexport {}\n\nexport {}\n",
        DeviceAccess::decl(&config),
        AppInfo::decl(&config),
    );
    declarations.push_str(&crate::keyboard::typescript_contracts(&config));
    use crate::automation::*;
    for d in [
        ActionCommand::decl(&config),
        ApplicationId::decl(&config),
        ActionStep::decl(&config),
        ActionDefinition::decl(&config),
        GestureRule::decl(&config),
        crate::exclusive_depth::DepthChoice::decl(&config),
        crate::exclusive_depth::HoldRepeat::decl(&config),
        AutomationProfile::decl(&config),
        PlatformCommands::decl(&config),
    ] {
        declarations.push_str(&format!("\nexport {d}\n"));
    }
    format!("{}\n", declarations.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_typescript_matches_rust_contract() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src/shared/contracts/generated.ts");
        let committed = std::fs::read_to_string(path).expect("Выполните npm run contracts");
        assert_eq!(committed.replace("\r\n", "\n"), typescript_contracts());
    }

    #[test]
    fn ipc_reports_transport_availability_without_claiming_a_connection() {
        let info = AppInfo {
            name: "IO Type 84".into(),
            version: "0.1.0".into(),
            platform: "windows".into(),
            device_access: DeviceAccess::Available,
        };
        let json = serde_json::to_value(info).unwrap();
        assert_eq!(json["deviceAccess"], "available");
        assert!(json.get("device_access").is_none());
        assert!(json.get("connected").is_none());
    }
}
