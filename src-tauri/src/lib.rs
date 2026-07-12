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
use walkdir::WalkDir;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
#[cfg(target_os = "windows")]
use {window_vibrancy::apply_acrylic, window_vibrancy::apply_mica, windows_version::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let maximum_concurrency = System::physical_core_count().unwrap_or(2).max(1);
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
        .plugin(tauri_plugin_fs::init())
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
            get_cpus,
            scan_dir,
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

    let shutdown_started = AtomicBool::new(false);
    app.run(move |_app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. })
            && !shutdown_started.swap(true, Ordering::AcqRel)
        {
            let _ = shutdown_service.manager().shutdown(Duration::from_secs(5));
        }
    });
}

#[tauri::command]
fn get_cpus() -> usize {
    System::physical_core_count().unwrap_or(2).max(1)
}

/// Compatibility scanner retained until drag/drop is fully moved to the
/// formal scan_inputs contract. Errors are skipped instead of panicking the
/// Tauri process.
#[tauri::command]
fn scan_dir(path: &str) -> Vec<String> {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.path().display().to_string())
        .collect()
}
