#[tauri::command]
fn app_info() -> io_core::application::AppInfo {
    io_platform::application_info()
}

use io_core::keyboard::*;
use io_platform::service::KeyboardService;
use tauri::Manager;

async fn background<T: Send + 'static>(
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|_| AppError::new("taskFailed", "Операция приложения прервана."))?
}

#[tauri::command]
async fn connect_device(service: tauri::State<'_, KeyboardService>) -> Result<KeyboardSnapshot> {
    let s = service.inner().clone();
    background(move || s.connect()).await
}
#[tauri::command]
async fn disconnect_device(service: tauri::State<'_, KeyboardService>) -> Result<()> {
    let s = service.inner().clone();
    background(move || s.disconnect()).await
}
#[tauri::command]
async fn refresh_device(service: tauri::State<'_, KeyboardService>) -> Result<KeyboardSnapshot> {
    let s = service.inner().clone();
    background(move || s.refresh()).await
}
#[tauri::command]
async fn set_monitor(
    enabled: bool,
    service: tauri::State<'_, KeyboardService>,
) -> Result<MonitorFrame> {
    let s = service.inner().clone();
    background(move || s.monitor(enabled)).await
}
#[tauri::command]
fn monitor_frame(service: tauri::State<'_, KeyboardService>) -> MonitorFrame {
    service.monitor_frame()
}
#[tauri::command]
fn clear_history(service: tauri::State<'_, KeyboardService>) {
    service.clear_history();
}
#[tauri::command]
fn set_history_capacity(count: usize, service: tauri::State<'_, KeyboardService>) {
    service.set_history_capacity(count);
}
#[tauri::command]
async fn prepare_changes(
    request: ChangeRequest,
    service: tauri::State<'_, KeyboardService>,
) -> Result<ChangePreview> {
    let s = service.inner().clone();
    background(move || s.prepare(request)).await
}
#[tauri::command]
async fn apply_changes(
    token: String,
    service: tauri::State<'_, KeyboardService>,
) -> Result<ApplyResult> {
    let s = service.inner().clone();
    background(move || s.apply(token)).await
}
#[tauri::command]
async fn prepare_recovery(service: tauri::State<'_, KeyboardService>) -> Result<ChangePreview> {
    let s = service.inner().clone();
    background(move || s.prepare_recovery()).await
}

#[tauri::command]
fn validate_automation(profile: io_core::automation::AutomationProfile) -> Result<()> {
    profile.validate()
}
#[tauri::command]
async fn configure_automation(
    profile: io_core::automation::AutomationProfile,
    enabled: bool,
    ownership_token: Option<String>,
    service: tauri::State<'_, KeyboardService>,
) -> Result<()> {
    let s = service.inner().clone();
    background(move || s.configure_automation(profile, enabled, ownership_token)).await
}

#[tauri::command]
async fn prepare_automation(
    profile: io_core::automation::AutomationProfile,
    base_revision: String,
    service: tauri::State<'_, KeyboardService>,
) -> Result<Option<ChangePreview>> {
    let s = service.inner().clone();
    background(move || s.prepare_automation(profile, base_revision)).await
}

fn app_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            app.manage(KeyboardService::new(
                app.path().app_data_dir()?.join("recovery"),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            connect_device,
            disconnect_device,
            refresh_device,
            set_monitor,
            monitor_frame,
            clear_history,
            set_history_capacity,
            configure_automation,
            prepare_automation,
            validate_automation,
            prepare_changes,
            apply_changes,
            prepare_recovery
        ])
        .build(app_context())
        .expect("Не удалось запустить IO Type 84");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            let _ = app.state::<KeyboardService>().disconnect();
        }
    });
}

#[cfg(all(test, feature = "custom-protocol"))]
mod tests {
    #[test]
    fn packaged_app_contains_frontend_without_a_dev_server() {
        assert!(!tauri::is_dev());
        let context = super::app_context();
        let html = context
            .assets()
            .get(&"index.html".into())
            .expect("Сборка должна содержать dist/index.html");
        assert!(String::from_utf8_lossy(&html).contains("id=\"app\""));
        assert!(context.assets().iter().any(|(key, _)| key.ends_with(".js")));
        assert!(
            context
                .assets()
                .iter()
                .any(|(key, _)| key.ends_with(".css"))
        );
    }
}
