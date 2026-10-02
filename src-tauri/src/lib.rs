// AppError and CommandErrorEnvelope are stable, serde-backed value contracts.
// Boxing every Result error would spread an internal size concern through the
// engine, scheduler, and Tauri command APIs without changing IPC payload size.
#![allow(clippy::result_large_err)]

pub mod backend;
pub mod domain;
pub mod engine;
pub mod jobs;
mod tauri_adapter;

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use backend::BackendService;
use sysinfo::System;
use tauri::Manager;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
#[cfg(target_os = "windows")]
use {window_vibrancy::apply_acrylic, window_vibrancy::apply_mica, windows_version::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let maximum_concurrency = available_physical_cores();
    let service = BackendService::new(maximum_concurrency)
        .expect("failed to initialize the neo-rimage backend");
    service
        .manager()
        .set_scheduler_paused(true)
        .expect("failed to initialize the scheduler in a paused state");
    let bridge_service = service.clone();
    let shutdown_service = service.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(service)
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            #[cfg(target_os = "macos")]
            apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None)
                .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");

            #[cfg(target_os = "windows")]
            {
                if OsVersion::current().build >= 22000 {
                    apply_mica(&window, Some(false))
                        .expect("Unsupported platform! 'apply_mica' is only supported on Windows");
                } else {
                    apply_acrylic(&window, Some((248, 249, 253, 220))).expect(
                        "Unsupported platform! 'apply_acrylic' is only supported on Windows",
                    );
                }
            }

            tauri_adapter::spawn_revision_bridge(app.handle().clone(), bridge_service.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_adapter::scan_inputs,
            tauri_adapter::get_backend_capabilities,
            tauri_adapter::get_backend_snapshot,
            tauri_adapter::get_job_snapshot,
            tauri_adapter::create_job,
            tauri_adapter::pause_job,
            tauri_adapter::resume_job,
            tauri_adapter::cancel_job,
            tauri_adapter::retry_job_items,
            tauri_adapter::remove_job,
            tauri_adapter::set_scheduler_paused,
            tauri_adapter::set_worker_count,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Tauri may report more than one exit request during teardown. Only the
    // first request should initiate the blocking cooperative-shutdown wait.
    let shutdown_started = AtomicBool::new(false);
    app.run(move |_app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. })
            && !shutdown_started.swap(true, Ordering::AcqRel)
        {
            let _ = shutdown_service.manager().shutdown(Duration::from_secs(5));
        }
    });
}

fn available_physical_cores() -> usize {
    System::physical_core_count().unwrap_or(2).max(1)
}
