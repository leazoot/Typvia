// SPIKE (TASK-005): throwaway Tauri 2 shell validation.
// Measures: tray creation, global shortcut registration, resident-hidden
// panel show latency, and on-demand window creation latency (OQ-A6).
// Results are printed as JSON on stdout; the app exits by itself.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

struct Acks {
    shown: Mutex<Option<Sender<u64>>>,
    ready: Mutex<Option<Sender<String>>>,
}

#[tauri::command]
fn ack_shown(state: State<Acks>, seq: u64) {
    if let Some(tx) = state.shown.lock().unwrap().as_ref() {
        let _ = tx.send(seq);
    }
}

#[tauri::command]
fn ack_ready(state: State<Acks>, label: String) {
    if let Some(tx) = state.ready.lock().unwrap().as_ref() {
        let _ = tx.send(label);
    }
}

fn stats(mut samples: Vec<f64>) -> serde_json::Value {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    serde_json::json!({
        "samples_ms": samples.iter().map(|v| (v * 10.0).round() / 10.0).collect::<Vec<_>>(),
        "min_ms": samples.first().copied().unwrap(),
        "median_ms": median,
        "max_ms": samples.last().copied().unwrap(),
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Acks { shown: Mutex::new(None), ready: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![ack_shown, ack_ready])
        .setup(|app| {
            let mut report = serde_json::Map::new();

            // 1. Tray icon.
            let tray = tauri::tray::TrayIconBuilder::new()
                .title("Tv")
                .tooltip("Typvia spike tray")
                .build(app);
            report.insert("tray_created".into(), serde_json::json!(tray.is_ok()));
            if let Err(e) = &tray {
                report.insert("tray_error".into(), serde_json::json!(e.to_string()));
            }

            // 2. Global shortcut registration (toggles the panel when pressed).
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
            let shortcut: Shortcut = "cmd+alt+k".parse().unwrap();
            let handle_for_shortcut = app.handle().clone();
            let reg = app.global_shortcut().on_shortcut(shortcut, move |_app, _sc, _event| {
                if let Some(panel) = handle_for_shortcut.get_webview_window("panel") {
                    if panel.is_visible().unwrap_or(false) {
                        let _ = panel.hide();
                    } else {
                        let _ = panel.show();
                        let _ = panel.set_focus();
                    }
                }
            });
            report.insert("global_shortcut_registered".into(), serde_json::json!(reg.is_ok()));
            if let Err(e) = &reg {
                report.insert("global_shortcut_error".into(), serde_json::json!(e.to_string()));
            }

            // 3. Latency measurements run off the main thread once the panel
            //    webview has loaded (it signals via the ready ack on page load
            //    only for cold windows, so we wait a fixed settle delay here).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                let acks = handle.state::<Acks>();
                let mut report = serde_json::Map::new();

                // 3a. Resident hidden panel: show -> painted frame ack.
                let panel = handle.get_webview_window("panel").expect("panel window exists");
                let mut resident = Vec::new();
                for seq in 0..20u64 {
                    let (tx, rx) = channel();
                    *acks.shown.lock().unwrap() = Some(tx);
                    let t0 = Instant::now();
                    panel.show().expect("panel show");
                    panel.emit_to("panel", "measure-show", seq).expect("emit measure");
                    match rx.recv_timeout(Duration::from_secs(5)) {
                        Ok(_) => resident.push(t0.elapsed().as_secs_f64() * 1000.0),
                        Err(_) => break,
                    }
                    panel.hide().expect("panel hide");
                    std::thread::sleep(Duration::from_millis(120));
                }
                report.insert(
                    "resident_hidden_show".into(),
                    if resident.is_empty() {
                        serde_json::json!("no acks received")
                    } else {
                        stats(resident)
                    },
                );

                // 3b. On-demand creation: build -> painted frame ack.
                let mut cold = Vec::new();
                for i in 0..5 {
                    let label = format!("cold{i}");
                    let (tx, rx) = channel();
                    *acks.ready.lock().unwrap() = Some(tx);
                    let t0 = Instant::now();
                    let win = WebviewWindowBuilder::new(
                        &handle,
                        &label,
                        WebviewUrl::App("index.html".into()),
                    )
                    .title("cold spike")
                    .inner_size(640.0, 420.0)
                    .visible(true)
                    .build()
                    .expect("cold window build");
                    match rx.recv_timeout(Duration::from_secs(10)) {
                        Ok(_) => cold.push(t0.elapsed().as_secs_f64() * 1000.0),
                        Err(_) => break,
                    }
                    win.close().expect("cold window close");
                    std::thread::sleep(Duration::from_millis(200));
                }
                report.insert(
                    "on_demand_create".into(),
                    if cold.is_empty() { serde_json::json!("no acks received") } else { stats(cold) },
                );

                println!(
                    "SPIKE_RESULT {}",
                    serde_json::Value::Object(report)
                );
                handle.exit(0);
            });

            println!("SPIKE_SETUP {}", serde_json::Value::Object(report));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri app run");
}
