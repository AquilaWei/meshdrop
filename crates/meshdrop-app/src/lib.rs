//! Desktop commands delegate to a testable message-driven application service.
#[cfg(target_os = "android")]
mod android;
pub mod service;
use service::Controller;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
async fn desktop_request(
    app: tauri::AppHandle,
    state: tauri::State<'_, Controller>,
    operation: String,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    if let Some(result) = android::request(&app, &state, &operation, payload.clone()).await {
        return result;
    }
    if operation == "pick_files" {
        let paths = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_files())
            .await
            .map_err(|e| e.to_string())?;
        if let Some(paths) = paths {
            let paths = paths
                .into_iter()
                .map(|path| path.into_path().map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            return state.call("select_files", serde_json::json!(paths)).await;
        }
        return Ok(serde_json::Value::Null);
    }
    if operation == "open_receive_directory" && cfg!(target_os = "android") {
        return Err("Opening receive folders is desktop-only; Android files are stored in app-private storage".into());
    }
    if operation == "open_receive_directory" {
        let path = state
            .call("receive_directory", serde_json::Value::Null)
            .await?;
        app.opener()
            .open_path(
                path.as_str().ok_or("invalid receive directory")?,
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
        return Ok(serde_json::Value::Null);
    }
    state.call(&operation, payload).await
}
/// Starts the native shell; setup errors are shown by Tauri and abort startup.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(android::init())
        .plugin(tauri_plugin_barcode_scanner::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (notices, mut events) = tokio::sync::mpsc::channel(16);
            let controller = tauri::async_runtime::block_on(Controller::start(
                if cfg!(target_os = "android") {
                    app.path().app_data_dir()?.join("files/meshdrop")
                } else {
                    service::state_directory()?
                },
                notices,
            ))?;
            app.manage(controller.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = events.recv().await {
                    let service::Notice::Invitation(body) = event;
                    if let Err(error) = handle
                        .notification()
                        .builder()
                        .title("meshdrop invitation")
                        .body(body)
                        .show()
                    {
                        let _ = controller
                            .call(
                                "notice_error",
                                serde_json::json!(format!("System notification: {error}")),
                            )
                            .await;
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![desktop_request])
        .on_window_event(|window, event| {
            if !cfg!(target_os = "android")
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
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
