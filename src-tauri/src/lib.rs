#[tauri::command]
fn app_info() -> io_core::application::AppInfo {
    io_platform::application_info()
}

fn app_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(app_context())
        .expect("Не удалось запустить IO Type 84");
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
