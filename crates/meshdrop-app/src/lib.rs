//! Desktop commands delegate to a testable message-driven application service.
pub mod service;
use service::Controller;
use tauri::Manager;

#[tauri::command]
async fn desktop_request(
    state: tauri::State<'_, Controller>,
    operation: String,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    state.call(&operation, payload).await
}
/// Starts the native shell; setup errors are shown by Tauri and abort startup.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (notices, mut events) = tokio::sync::mpsc::channel(16);
            let controller = tauri::async_runtime::block_on(Controller::start(
                service::state_directory()?,
                notices,
            ))?;
            app.manage(controller);
            tauri::async_runtime::spawn(async move {
                while let Some(event) = events.recv().await {
                    eprintln!("desktop notice: {event:?}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![desktop_request])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                let controller = app.state::<Controller>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    controller.shutdown().await;
                    app.exit(0);
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("run meshdrop desktop application");
}
