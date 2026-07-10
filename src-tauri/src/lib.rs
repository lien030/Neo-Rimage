mod domain;
mod engine;
mod jobs;
mod models;
use crossbeam_channel::{bounded, select, unbounded, Receiver, Sender};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use std::thread;
use sysinfo::System;

use crate::models::{ProcessWorker, SimpleWorker, Task, WorkerStatus};
use tauri::Manager;
use walkdir::WalkDir;
#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
#[cfg(target_os = "windows")]
use {window_vibrancy::apply_acrylic, window_vibrancy::apply_mica, windows_version::*};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            #[cfg(target_os = "macos")]
            apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None)
                .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");

            #[cfg(target_os = "windows")]
            {
                println!("Current version: {:#?}", OsVersion::current());
                if OsVersion::current().build >= 22000 {
                    apply_mica(&window, Some(false))
                        .expect("Unsupported platform! 'apply_mica' is only supported on Windows");
                } else {
                    apply_acrylic(&window, Some((248, 249, 253, 220)))
                        .expect("Unsupported platform! 'apply_blur' is only supported on Windows");
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            get_cpus,
            scan_dir,
            add_worker,
            remove_worker,
            get_workers,
            get_workers_len,
            add_task,
            get_tasks_len,
            clear_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_cpus() -> usize {
    let res = System::physical_core_count();
    res.unwrap_or(2)
}

#[tauri::command]
fn scan_dir(path: &str) -> Vec<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(path) {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            files.push(entry.path().display().to_string());
        }
    }
    files
}

static WORKER_HANDLES: Lazy<Arc<Mutex<Vec<ProcessWorker>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

static TASK_LIST_SENDER_RECEIVER: Lazy<(Sender<Task>, Receiver<Task>)> = Lazy::new(|| unbounded());

#[tauri::command]
fn add_worker(worker_id: &str) -> bool {
    println!("Adding worker {}", worker_id);
    let (_, ref r1) = *TASK_LIST_SENDER_RECEIVER;
    let rx = r1.clone();
    let worker_id_clone = worker_id.to_string();
    let (stop_tx, stop_rx) = bounded(1);
    let handles_clone = Arc::clone(&WORKER_HANDLES);
    let mut workers = handles_clone.lock().unwrap();
    let handle = thread::spawn(move || {
        loop {
            select! {
                recv(rx) -> msg => {
                    match msg {
                        Ok(task) => {
                            println!("Worker {} is processing task {}", worker_id_clone, task.id);
                            // let mut workers = workers_clone.lock().unwrap();
                            // let worker = workers.iter().position(|w| w.id == worker_id_clone);
                            // if let Some(worker) = worker {
                            //     workers[worker].status = Status::Busy;
                            //     workers[worker].task = Some(task);
                            // }
                            // thread::sleep(std::time::Duration::from_secs(5));
                            // let mut workers = workers_clone.lock().unwrap();
                            // let worker = workers.iter().position(|w| w.id == worker_id_clone);
                            // if let Some(worker) = worker {
                            //     workers[worker].status = Status::Done;
                            //     workers[worker].task = None;
                            // }
                            process_image(&task);
                            thread::sleep(std::time::Duration::from_secs(1));
                        }
                        Err(_) => {}
                    }
                }
                recv(stop_rx) -> _ => {
                    println!("Worker {} is stopping", worker_id_clone);
                    break;
                }
            }
        }
    });
    workers.push(ProcessWorker {
        id: worker_id.to_string(),
        status: WorkerStatus::Idle,
        handle,
        control: stop_tx.clone(),
        task: None,
    });
    true
}

#[tauri::command]
fn remove_worker(worker_id: &str) -> bool {
    println!("Removing worker {}", worker_id);
    let handles_clone = Arc::clone(&WORKER_HANDLES);
    let mut workers = handles_clone.lock().unwrap();
    let worker = workers.iter().position(|w| w.id == worker_id);
    if let Some(worker) = worker {
        workers[worker].control.send(()).unwrap();
        workers.remove(worker);
    } else {
        return false;
    }
    true
}

#[tauri::command]
fn get_workers_len() -> usize {
    let handles_clone = Arc::clone(&WORKER_HANDLES);
    let workers = handles_clone.lock().unwrap();
    workers.len()
}

#[tauri::command]
fn get_workers() -> Vec<SimpleWorker> {
    let handles_clone = Arc::clone(&WORKER_HANDLES);
    let workers = handles_clone.lock().unwrap();
    workers
        .iter()
        .map(|w| SimpleWorker {
            id: w.id.clone(),
            status: w.status as usize,
            task_id: w.task.as_ref().map_or("".to_string(), |t| t.id.clone()),
        })
        .collect()
}

#[tauri::command]
fn add_task(tasks: Vec<Task>) -> bool {
    let (ref s1, ref r1) = *TASK_LIST_SENDER_RECEIVER;
    let tx = s1.clone();
    for t in &tasks {
        match tx.send(Task {
            id: t.id.clone(),
            status: t.status.clone(),
            file_name: t.file_name.clone(),
            file_path: t.file_path.clone(),
        }) {
            Ok(_) => {
                println!("Added task id: {}, filename: {}", t.id, t.file_name);
            }
            Err(_) => return false,
        }
    }
    println!(
        "Added {} tasks, Channel len: {}",
        tasks.len(),
        r1.clone().len()
    );
    true
}

#[tauri::command]
fn get_tasks_len() -> usize {
    let (_, ref r1) = *TASK_LIST_SENDER_RECEIVER;
    let rx = r1.clone();
    rx.len()
}

#[tauri::command]
async fn clear_tasks() {
    let (_, ref r1) = *TASK_LIST_SENDER_RECEIVER;
    let rx = r1.clone();
    while let Ok(task) = rx.try_recv() {
        println!("Removed task id: {}, filename: {}", task.id, task.file_name)
    }
}

fn process_image(task: &Task) {
    println!("Processing image {}", task.id);
}
