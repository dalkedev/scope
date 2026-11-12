mod service;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use service::AppCore;
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

fn argument(name: &str) -> Option<String> {
    let args = std::env::args().collect::<Vec<_>>();
    args.iter()
        .position(|s| s == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
fn data_dir() -> PathBuf {
    argument("--data-dir")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("SCOPE_DATA_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("dev", "scope", "Scope")
                .expect("user data directory")
                .data_dir()
                .to_path_buf()
        })
}
fn port() -> u16 {
    argument("--bridge-port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(17892)
}
fn string<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing {key}"))
}
fn boolean(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

async fn execute(app: &AppCore, command: &str, args: Value) -> Result<Value> {
    let e = &app.engine;
    Ok(match command {
        "bootstrap" => app.bootstrap()?,
        "list_sessions" => serde_json::to_value(e.list_sessions()?)?,
        "list_rules" => serde_json::to_value(e.list_rules()?)?,
        "save_rule" => serde_json::to_value(
            e.save_rule(serde_json::from_value(
                args.get("rule")
                    .cloned()
                    .ok_or_else(|| anyhow!("Missing rule"))?,
            )?)?,
        )?,
        "delete_rule" => {
            e.delete_rule(string(&args, "id")?)?;
            json!({"ok":true})
        }
        "update_session" => serde_json::to_value(
            e.update_session(
                string(&args, "id")?,
                serde_json::from_value(
                    args.get("update")
                        .cloned()
                        .ok_or_else(|| anyhow!("Missing update"))?,
                )?,
            )?,
        )?,
        "delete_session" => {
            e.delete_session(string(&args, "id")?)?;
            json!({"ok":true})
        }
        "list_audit" => serde_json::to_value(
            e.list_audit(
                args.get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(200)
                    .min(1000) as usize,
            )?,
        )?,
        "create_session" => serde_json::to_value(
            e.create_session(
                string(&args, "name")?.to_string(),
                args.get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("manual")
                    .to_string(),
            )?,
        )?,
        "list_exchanges" => {
            let mut value = serde_json::to_value(
                e.list_exchanges(
                    string(&args, "sessionId")?,
                    args.get("query").and_then(Value::as_str),
                    args.get("limit")
                        .and_then(Value::as_u64)
                        .unwrap_or(250)
                        .min(1000) as usize,
                    args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize,
                )?,
            )?;
            if let Some(rows) = value.as_array_mut() {
                for row in rows {
                    if let Some(map) = row.as_object_mut() {
                        map.insert("requestBody".into(), json!(""));
                        map.insert("responseBody".into(), json!(""));
                        map.remove("originalRequest");
                        map.remove("originalResponse");
                    }
                }
            }
            value
        }
        "get_exchange" => serde_json::to_value(e.get_exchange(string(&args, "id")?)?)?,
        "analysis" => e.analysis(string(&args, "sessionId")?)?,
        "infer_openapi" => e.infer_openapi(string(&args, "sessionId")?)?,
        "export_har" => e.export_har(string(&args, "sessionId")?)?,
        "export_session" => e.export_session(string(&args, "sessionId")?)?,
        "import_har" => serde_json::to_value(
            e.import_har(
                serde_json::from_str(string(&args, "content")?)?,
                args.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Imported HAR")
                    .to_string(),
            )?,
        )?,
        "import_session" => serde_json::to_value(
            e.import_session(serde_json::from_str(string(&args, "content")?)?)?,
        )?,
        "proxy_status" => serde_json::to_value(e.proxy_status())?,
        "start_proxy" => serde_json::to_value(
            e.start_proxy(
                string(&args, "sessionId")?.to_string(),
                args.get("port")
                    .and_then(Value::as_u64)
                    .unwrap_or(17890)
                    .try_into()?,
            )
            .await?,
        )?,
        "stop_proxy" => {
            app.devices.stop().await?;
            e.stop_proxy().await?;
            serde_json::to_value(e.proxy_status())?
        }
        "set_breakpoints" => {
            e.record_audit("breakpoint.configure", None)?;
            e.set_breakpoints(boolean(&args, "request"), boolean(&args, "response"));
            serde_json::to_value(e.proxy_status())?
        }
        "pending" => serde_json::to_value(e.pending())?,
        "resolve_breakpoint" => {
            e.record_audit("breakpoint.resolve", Some(string(&args, "id")?))?;
            e.resolve_breakpoint(
                string(&args, "id")?,
                serde_json::from_value(
                    args.get("edit")
                        .cloned()
                        .ok_or_else(|| anyhow!("Missing edit"))?,
                )?,
            )?;
            json!({"ok":true})
        }
        "replay" => serde_json::to_value(
            e.replay(
                string(&args, "id")?,
                args.get("edit")
                    .cloned()
                    .filter(|v| !v.is_null())
                    .map(serde_json::from_value)
                    .transpose()?,
            )
            .await?,
        )?,
        "compare_sessions" => {
            e.compare_sessions(string(&args, "left")?, string(&args, "right")?)?
        }
        "list_devices" => serde_json::to_value(app.devices.list_devices().await?)?,
        "device_diagnostics" => serde_json::to_value(
            app.devices
                .diagnostics(
                    args.get("serial")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
                .await?,
        )?,
        "list_apps" => {
            serde_json::to_value(app.devices.list_apps(string(&args, "serial")?).await?)?
        }
        "device_status" => serde_json::to_value(app.devices.status())?,
        "start_device" => {
            let proxy = e.proxy_status();
            anyhow::ensure!(
                proxy.running,
                "Start a capture session before connecting an Android app"
            );
            e.record_audit("device.start", None)?;
            serde_json::to_value(
                app.devices
                    .start(
                        string(&args, "serial")?.to_string(),
                        string(&args, "package")?.to_string(),
                        string(&args, "mode")?.to_string(),
                        proxy.port,
                        e.ca_cert_path(),
                        serde_json::from_value(args.get("profile").cloned().unwrap_or(json!([])))?,
                    )
                    .await?,
            )?
        }
        "stop_device" => {
            e.record_audit("device.stop", None)?;
            serde_json::to_value(app.devices.stop().await?)?
        }
        "set_mcp_scope" => app.set_mcp_scope(
            serde_json::from_value(args.get("sessionIds").cloned().unwrap_or(json!([])))?,
            boolean(&args, "allowWrites"),
            boolean(&args, "allowRules"),
            boolean(&args, "allowDeviceDiagnostics"),
        )?,
        "generate_demo" => {
            app.generate_demo(string(&args, "sessionId")?.to_string())
                .await?
        }
        _ => anyhow::bail!("Unknown Snurpo command: {command}"),
    })
}

#[tauri::command]
async fn scope_command(
    app: tauri::State<'_, Arc<AppCore>>,
    handle: tauri::AppHandle,
    command: String,
    args: Option<Value>,
) -> std::result::Result<Value, String> {
    let args = args.unwrap_or(json!({}));
    if command == "save_export" {
        return save_export(app.inner(), handle, args)
            .await
            .map_err(|e| format!("{e:#}"));
    }
    execute(app.inner(), &command, args)
        .await
        .map_err(|e| format!("{e:#}"))
}

async fn save_export(app: &AppCore, handle: tauri::AppHandle, args: Value) -> Result<Value> {
    let id = string(&args, "sessionId")?;
    let format = string(&args, "format")?;
    anyhow::ensure!(
        matches!(format, "har" | "scope" | "openapi"),
        "Unknown export format"
    );
    let mut value = if format == "har" {
        app.engine.export_har(id)?
    } else if format == "openapi" {
        app.engine.infer_openapi(id)?
    } else {
        app.engine.export_session(id)?
    };
    if boolean(&args, "redact") && format != "openapi" {
        value = service::sanitize_archive(value);
    }
    let extension = if format == "har" { "har" } else { "json" };
    let name = format!(
        "scope-{}.{}",
        &id[..id.len().min(8)],
        match format {
            "har" => "har",
            "openapi" => "openapi.json",
            _ => "scope.json",
        }
    );
    let (tx, rx) = tokio::sync::oneshot::channel();
    handle
        .dialog()
        .file()
        .set_title("Export captured session")
        .set_file_name(name)
        .add_filter("Capture archive", &[extension])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let path = rx.await?;
    let Some(path) = path else {
        return Ok(json!({"saved":false}));
    };
    let path = path
        .into_path()
        .map_err(|e| anyhow!("Invalid export path: {e}"))?;
    service::write_private(&path, &serde_json::to_vec_pretty(&value)?)?;
    app.engine.record_audit("session.export", Some(id))?;
    Ok(json!({"saved":true,"path":path}))
}

fn app_context() -> tauri::Context<tauri::Wry> {
    // This macro embeds a macOS Info.plist symbol, so expand it only once.
    tauri::generate_context!()
}

pub async fn headless() -> Result<()> {
    // Use the same platform resource layout as the GUI, including relocated
    // .app bundles, Windows installs and Linux AppImages/deb installations.
    // The checkout fallback is only for unbundled Cargo development binaries.
    let context = app_context();
    let resources =
        tauri::utils::platform::resource_dir(context.package_info(), &tauri::Env::default())
            .ok()
            .filter(|root| root.join("resources").is_dir())
            .or_else(|| {
                let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()?
                    .to_path_buf();
                checkout.join("resources").is_dir().then_some(checkout)
            });
    let app = AppCore::open(data_dir(), resources, port()).await?;
    if std::env::args().any(|s| s == "--demo-server") {
        app.ensure_demo().await?;
    }
    if std::env::args().any(|s| s == "--demo") {
        let session = app
            .engine
            .create_session("Local demo".into(), "local-demo".into())?;
        app.generate_demo(session.id.clone()).await?;
        app.set_mcp_scope(vec![session.id], false, false, false)?;
    }
    println!(
        "{}",
        serde_json::to_string(
            &json!({"status":"ready","dataDir":app.data_dir,"mcpConfig":serde_json::from_str::<Value>(&app.mcp_config())?,"bridge":app.bridge.endpoint})
        )?
    );
    tokio::signal::ctrl_c().await?;
    let _ = app.devices.stop().await;
    app.engine.stop_proxy().await?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_main_window(app: &tauri::AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    } else if let Some(config) = app.config().app.windows.first() {
        tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_menu_bar(app: &tauri::App) -> Result<()> {
    use tauri::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        tray::TrayIconBuilder,
    };

    let show = MenuItem::with_id(app, "snurpo.show", "Show Snurpo", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "snurpo.quit", "Quit Snurpo", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;
    TrayIconBuilder::with_id("snurpo-status")
        .icon(tauri::include_image!("icons/snurpo-tray-template.png"))
        .icon_as_template(true)
        .tooltip("Snurpo: A nosy little HTTP debugger.")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "snurpo.show" => {
                if let Err(error) = show_main_window(app) {
                    eprintln!("Snurpo: could not show the window: {error:#}");
                }
            }
            // AppHandle::exit goes through the existing RunEvent::Exit cleanup.
            "snurpo.quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scope_command])
        .setup(|app| {
            let resources = app.path().resource_dir().ok();
            let core =
                tauri::async_runtime::block_on(AppCore::open(data_dir(), resources, port()))?;
            app.manage(core);
            #[cfg(target_os = "macos")]
            setup_menu_bar(app)?;
            Ok(())
        })
        .build(app_context())
        .expect("build Snurpo desktop")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Err(error) = show_main_window(app) {
                    eprintln!("Snurpo: could not reopen the window: {error:#}");
                }
            }
            if let tauri::RunEvent::Exit = event {
                if let Some(core) = app.try_state::<Arc<AppCore>>() {
                    tauri::async_runtime::block_on(async {
                        let _ = core.devices.stop().await;
                        let _ = core.engine.stop_proxy().await;
                    });
                }
            }
        });
}
