// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use kintree_app::{dispatch, ApiError, Session};
use std::sync::Mutex;
use tauri::State;

struct AppState(Mutex<Session>);

/// The single IPC entry point: the UI sends `{ cmd, args }` and gets back JSON or `{ code, message }`.
/// `async` runs it off the main thread so long imports never freeze the window.
#[tauri::command(async)]
fn call(state: State<'_, AppState>, cmd: String, args: serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let mut session = state.0.lock().map_err(|_| ApiError::internal("session lock poisoned"))?;
    dispatch(&mut session, &cmd, args)
}

fn main() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(Session::new())))
        .invoke_handler(tauri::generate_handler![call])
        .run(tauri::generate_context!())
        .expect("error while running KinTree");
}
