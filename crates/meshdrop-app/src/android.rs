//! Thin Android lifecycle/content bridge; decisions and transfer state remain in core.
use crate::service::Controller;
use serde_json::{Value, json};
use tauri::{
    Manager,
    ipc::Channel,
    plugin::{Builder, PluginHandle, TauriPlugin},
};
struct Bridge(PluginHandle<tauri::Wry>);
/// Registers native lifecycle/content effects; registration failures abort setup.
pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("receiver")
        .setup(|app, api| {
            app.manage(Bridge(api.register_android_plugin(
                "org.meshdrop.desktop",
                "ReceiverPlugin",
            )?));
            Ok(())
        })
        .build()
}
/// Dispatches Android-only effects off the async executor; native errors reach the UI.
/// Returns None when the shared controller should handle the operation.
pub async fn request(
    app: &tauri::AppHandle,
    controller: &Controller,
    operation: &str,
    payload: Value,
) -> Option<Result<Value, String>> {
    if !matches!(
        operation,
        "android_start" | "android_stop" | "received_files" | "share_file" | "pick_files"
    ) {
        return None;
    }
    let app = app.clone();
    let controller = controller.clone();
    let operation = operation.to_owned();
    Some(
        tokio::task::spawn_blocking(move || {
            let bridge = app.state::<Bridge>();
            let result = match operation.as_str() {
                "android_start" => {
                    use tauri_plugin_notification::NotificationExt;
                    let permission = app
                        .notification()
                        .request_permission()
                        .map_err(|e| e.to_string())?;
                    let channel: Channel<Value> = Channel::new(move |event| {
                        if let tauri::ipc::InvokeResponseBody::Json(body) = event {
                            eprintln!("Android receiver lifecycle: {body}");
                        }
                        let receiver = controller.clone();
                        tauri::async_runtime::spawn(async move {
                            receiver.shutdown().await;
                        });
                        Ok(())
                    });
                    bridge
                        .0
                        .run_mobile_plugin::<Value>("start", json!({"channel":channel}))
                        .map_err(|e| e.to_string())?;
                    return Ok(json!({"permission":permission}));
                }
                "android_stop" => bridge.0.run_mobile_plugin("stop", json!({})),
                "received_files" => bridge.0.run_mobile_plugin("files", json!({})),
                "share_file" => bridge.0.run_mobile_plugin("share", payload),
                "pick_files" => {
                    let value: Value = bridge
                        .0
                        .run_mobile_plugin("pick", json!({}))
                        .map_err(|e| e.to_string())?;
                    return tauri::async_runtime::block_on(
                        controller.call("select_files", value["paths"].clone()),
                    );
                }
                _ => unreachable!(),
            };
            result.map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result),
    )
}
