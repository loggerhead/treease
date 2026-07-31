//! Treease Desktop Workspace Tauri host.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use keyring::Entry;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

const RECENT_FILES_FILENAME: &str = "desktop-recent-files.json";
const WORKSPACE_SESSION_FILENAME: &str = "desktop-workspace-session.json";
const KEYRING_SERVICE: &str = "com.treease.desktop";
const KEYRING_REFRESH_TOKEN_ACCOUNT: &str = "supabase-refresh-token";
const OPEN_RECENT_MENU_PREFIX: &str = "workspace:open-recent:";

#[derive(Deserialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileAccessGrant {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct OpenedFile {
    grant: FileAccessGrant,
    text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RecentFileRecord {
    id: String,
    name: String,
    path: PathBuf,
}

#[derive(Default)]
struct FileAccessState {
    grants: Mutex<HashMap<String, PathBuf>>,
    recent: Mutex<Vec<RecentFileRecord>>,
    watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    startup_files: Mutex<Vec<OpenedFile>>,
    next_id: AtomicU64,
}

fn open_explicit_paths(
    state: &FileAccessState,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Vec<OpenedFile> {
    paths
        .into_iter()
        .filter_map(|path| {
            let grant = state.grant(path).ok()?;
            let text = fs::read_to_string(state.path(&grant.id).ok()?).ok()?;
            Some(OpenedFile { grant, text })
        })
        .collect()
}

impl FileAccessState {
    fn grant(&self, path: PathBuf) -> Result<FileAccessGrant, String> {
        let path = path
            .canonicalize()
            .map_err(|error| format!("Cannot access the selected file: {error}"))?;
        if !path.is_file() {
            return Err("Only individual files can be opened.".into());
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "The selected file needs a valid name.".to_owned())?
            .to_owned();
        let id = format!("file-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        self.grants
            .lock()
            .map_err(|_| "File access state is unavailable.".to_owned())?
            .insert(id.clone(), path.clone());
        let grant = FileAccessGrant { id, name };
        let mut recent = self
            .recent
            .lock()
            .map_err(|_| "Recent file state is unavailable.".to_owned())?;
        recent.retain(|item| item.path != path);
        recent.insert(
            0,
            RecentFileRecord {
                id: format!("recent-{}", self.next_id.fetch_add(1, Ordering::Relaxed)),
                name: grant.name.clone(),
                path,
            },
        );
        recent.truncate(20);
        Ok(grant)
    }

    fn path(&self, grant_id: &str) -> Result<PathBuf, String> {
        self.grants
            .lock()
            .map_err(|_| "File access state is unavailable.".to_owned())?
            .get(grant_id)
            .cloned()
            .ok_or_else(|| "This file was not explicitly granted to Treease.".to_owned())
    }

    fn recent_files(&self) -> Result<Vec<FileAccessGrant>, String> {
        Ok(self
            .recent
            .lock()
            .map_err(|_| "Recent file state is unavailable.".to_owned())?
            .iter()
            .map(|file| FileAccessGrant {
                id: file.id.clone(),
                name: file.name.clone(),
            })
            .collect())
    }

    fn recent_path(&self, recent_id: &str) -> Result<PathBuf, String> {
        self.recent
            .lock()
            .map_err(|_| "Recent file state is unavailable.".to_owned())?
            .iter()
            .find(|file| file.id == recent_id)
            .map(|file| file.path.clone())
            .ok_or_else(|| "This recent file is no longer available.".to_owned())
    }

    fn clear_recent_files(&self) -> Result<(), String> {
        self.recent
            .lock()
            .map_err(|_| "Recent file state is unavailable.".to_owned())?
            .clear();
        Ok(())
    }
}

fn app_data_file(app: &AppHandle, filename: &str) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Cannot resolve the application data directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Cannot create the application data directory: {error}"))?;
    Ok(directory.join(filename))
}

fn write_json_atomically(
    app: &AppHandle,
    filename: &str,
    value: &impl Serialize,
) -> Result<(), String> {
    let destination = app_data_file(app, filename)?;
    write_json_atomically_at(&destination, value)
}

fn write_json_atomically_at(destination: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = destination.with_extension("tmp");
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("Cannot serialize application data: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Cannot write application data: {error}"))?;
    fs::rename(&temporary, destination)
        .map_err(|error| format!("Cannot finalize application data: {error}"))
}

fn read_json_if_present(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("Cannot read application data: {error}"))?,
    )
    .map(Some)
    .map_err(|error| format!("Cannot parse application data: {error}"))
}

fn load_recent_files(app: &AppHandle, state: &FileAccessState) -> Result<(), String> {
    let path = app_data_file(app, RECENT_FILES_FILENAME)?;
    if !path.exists() {
        return Ok(());
    }
    let records: Vec<RecentFileRecord> = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("Cannot read recent files: {error}"))?,
    )
    .map_err(|error| format!("Cannot parse recent files: {error}"))?;
    let mut recent = state
        .recent
        .lock()
        .map_err(|_| "Recent file state is unavailable.".to_owned())?;
    *recent = records
        .into_iter()
        .filter(|record| record.path.is_file())
        .take(20)
        .collect();
    Ok(())
}

fn persist_recent_files(app: &AppHandle, state: &FileAccessState) -> Result<(), String> {
    let recent = state
        .recent
        .lock()
        .map_err(|_| "Recent file state is unavailable.".to_owned())?
        .clone();
    write_json_atomically(app, RECENT_FILES_FILENAME, &recent)
}

fn persist_recent_files_and_refresh_menu(
    app: &AppHandle,
    state: &FileAccessState,
) -> Result<(), String> {
    // Recent-file ownership stays in the host; rebuilding keeps the native menu in sync
    // after every grant mutation without exposing paths to the Web workspace.
    persist_recent_files(app, state)?;
    let menu = create_application_menu(app, state).map_err(|error| error.to_string())?;
    app.set_menu(menu).map_err(|error| error.to_string())?;
    Ok(())
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| "The selected file needs a valid name.".to_owned())
}

fn refresh_token_entry() -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_TOKEN_ACCOUNT)
        .map_err(|error| format!("Cannot access the system credential store: {error}"))
}

#[tauri::command]
async fn store_refresh_token(refresh_token: String) -> Result<(), String> {
    if refresh_token.trim().is_empty() {
        return Err("A refresh token is required.".into());
    }
    refresh_token_entry()?
        .set_password(&refresh_token)
        .map_err(|error| {
            format!("Cannot save the refresh token in the system credential store: {error}")
        })
}

#[tauri::command]
async fn has_refresh_token() -> Result<bool, String> {
    match refresh_token_entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(error) => Err(format!("Cannot read the system credential store: {error}")),
    }
}

#[tauri::command]
async fn clear_refresh_token() -> Result<(), String> {
    match refresh_token_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Cannot remove the refresh token from the system credential store: {error}"
        )),
    }
}

fn supabase_refresh_endpoint(supabase_url: &str) -> Result<String, String> {
    let base = supabase_url
        .parse::<tauri::Url>()
        .map_err(|_| "The configured Supabase URL is invalid.".to_owned())?;
    let allowed_host = base
        .host_str()
        .is_some_and(|host| host.ends_with(".supabase.co"));
    if base.scheme() != "https" || !allowed_host || !matches!(base.path(), "" | "/") {
        return Err("The configured Supabase URL is not an approved HTTPS endpoint.".into());
    }
    Ok(format!(
        "{}/auth/v1/token?grant_type=refresh_token",
        base.origin().ascii_serialization()
    ))
}

#[tauri::command]
async fn refresh_access_token(
    supabase_url: String,
    anon_key: String,
) -> Result<serde_json::Value, String> {
    let endpoint = supabase_refresh_endpoint(&supabase_url)?;
    if anon_key.trim().is_empty() {
        return Err("The Supabase anonymous key is required.".into());
    }
    let refresh_token = refresh_token_entry()?
        .get_password()
        .map_err(|error| format!("Cannot read the system credential store: {error}"))?;
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("apikey", &anon_key)
        .header("authorization", format!("Bearer {anon_key}"))
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|_| "Cannot reach the configured Supabase service.".to_owned())?;
    if !response.status().is_success() {
        return Err("The stored desktop session is no longer valid.".into());
    }
    let tokens = response
        .json::<RefreshTokenResponse>()
        .await
        .map_err(|_| "The Supabase refresh response was invalid.".to_owned())?;
    if tokens.access_token.is_empty() || tokens.refresh_token.is_empty() {
        return Err("The Supabase refresh response was incomplete.".into());
    }
    refresh_token_entry()?
        .set_password(&tokens.refresh_token)
        .map_err(|error| format!("Cannot update the system credential store: {error}"))?;
    Ok(serde_json::json!({
        "accessToken": tokens.access_token,
        "refreshToken": tokens.refresh_token,
    }))
}

#[tauri::command]
async fn pick_file(
    app: AppHandle,
    state: State<'_, FileAccessState>,
) -> Result<Option<OpenedFile>, String> {
    let selected = app
        .dialog()
        .file()
        .add_filter(
            "Treease documents",
            &["json", "jsonl", "ndjson", "yaml", "yml", "toml", "csv"],
        )
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| "The selected file is not a local file.".to_owned())?;
    let grant = state.grant(path)?;
    persist_recent_files_and_refresh_menu(&app, &state)?;
    let text = std::fs::read_to_string(state.path(&grant.id)?)
        .map_err(|error| format!("Cannot read the selected file: {error}"))?;
    Ok(Some(OpenedFile { grant, text }))
}

#[tauri::command]
async fn read_granted_file(
    state: State<'_, FileAccessState>,
    grant_id: String,
) -> Result<OpenedFile, String> {
    let path = state.path(&grant_id)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read the granted file: {error}"))?;
    Ok(OpenedFile {
        grant: FileAccessGrant {
            id: grant_id,
            name: file_name(&path)?,
        },
        text,
    })
}

#[tauri::command]
async fn save_granted_file(
    state: State<'_, FileAccessState>,
    grant_id: String,
    text: String,
) -> Result<(), String> {
    let path = state.path(&grant_id)?;
    std::fs::write(path, text).map_err(|error| format!("Cannot save the granted file: {error}"))
}

#[tauri::command]
async fn save_new_file(
    app: AppHandle,
    state: State<'_, FileAccessState>,
    file_name: String,
    text: String,
) -> Result<Option<FileAccessGrant>, String> {
    let selected = app
        .dialog()
        .file()
        .set_file_name(file_name)
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| "The selected file is not a local file.".to_owned())?;
    std::fs::write(&path, text)
        .map_err(|error| format!("Cannot save the selected file: {error}"))?;
    let grant = state.grant(path)?;
    persist_recent_files_and_refresh_menu(&app, &state)?;
    Ok(Some(grant))
}

#[tauri::command]
async fn open_recent_file(
    app: AppHandle,
    state: State<'_, FileAccessState>,
    recent_id: String,
) -> Result<OpenedFile, String> {
    let path = state.recent_path(&recent_id)?;
    let grant = state.grant(path)?;
    persist_recent_files_and_refresh_menu(&app, &state)?;
    let text = std::fs::read_to_string(state.path(&grant.id)?)
        .map_err(|error| format!("Cannot read the recent file: {error}"))?;
    Ok(OpenedFile { grant, text })
}

#[tauri::command]
async fn take_startup_files(state: State<'_, FileAccessState>) -> Result<Vec<OpenedFile>, String> {
    let mut startup_files = state
        .startup_files
        .lock()
        .map_err(|_| "Startup file state is unavailable.".to_owned())?;
    Ok(std::mem::take(&mut *startup_files))
}

#[tauri::command]
async fn watch_granted_file(
    app: AppHandle,
    state: State<'_, FileAccessState>,
    grant_id: String,
) -> Result<(), String> {
    let path = state.path(&grant_id)?;
    let event_grant_id = grant_id.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result.is_ok() {
            let _ = app.emit(
                "workspace-file-changed",
                serde_json::json!({ "grantId": event_grant_id }),
            );
        }
    })
    .map_err(|error| format!("Cannot watch the granted file: {error}"))?;
    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|error| format!("Cannot watch the granted file: {error}"))?;
    state
        .watchers
        .lock()
        .map_err(|_| "File watcher state is unavailable.".to_owned())?
        .insert(grant_id, watcher);
    Ok(())
}

#[tauri::command]
async fn unwatch_granted_file(
    state: State<'_, FileAccessState>,
    grant_id: String,
) -> Result<(), String> {
    state
        .watchers
        .lock()
        .map_err(|_| "File watcher state is unavailable.".to_owned())?
        .remove(&grant_id);
    Ok(())
}

#[tauri::command]
async fn list_recent_files(
    state: State<'_, FileAccessState>,
) -> Result<Vec<FileAccessGrant>, String> {
    state.recent_files()
}

#[tauri::command]
async fn clear_recent_files(
    app: AppHandle,
    state: State<'_, FileAccessState>,
) -> Result<(), String> {
    state.clear_recent_files()?;
    persist_recent_files_and_refresh_menu(&app, &state)
}

#[tauri::command]
async fn save_workspace_session(app: AppHandle, session: serde_json::Value) -> Result<(), String> {
    validate_workspace_session(&session)?;
    write_json_atomically(&app, WORKSPACE_SESSION_FILENAME, &session)
}

fn validate_workspace_session(session: &serde_json::Value) -> Result<(), String> {
    let Some(version) = session.get("version").and_then(serde_json::Value::as_u64) else {
        return Err("The workspace session must include a version.".into());
    };
    if version != 1 {
        return Err("This workspace session version is not supported.".into());
    }
    if !session.get("tabs").is_some_and(serde_json::Value::is_array) {
        return Err("The workspace session must include tabs.".into());
    }
    Ok(())
}

#[tauri::command]
async fn load_workspace_session(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    let path = app_data_file(&app, WORKSPACE_SESSION_FILENAME)?;
    read_json_if_present(&path)
}

#[tauri::command]
async fn reset_application_data(
    app: AppHandle,
    state: State<'_, FileAccessState>,
) -> Result<(), String> {
    {
        let mut grants = state
            .grants
            .lock()
            .map_err(|_| "File access state is unavailable.".to_owned())?;
        grants.clear();
        let mut recent = state
            .recent
            .lock()
            .map_err(|_| "Recent file state is unavailable.".to_owned())?;
        recent.clear();
        let mut startup_files = state
            .startup_files
            .lock()
            .map_err(|_| "Startup file state is unavailable.".to_owned())?;
        startup_files.clear();
        let mut watchers = state
            .watchers
            .lock()
            .map_err(|_| "File watcher state is unavailable.".to_owned())?;
        watchers.clear();
    }

    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Cannot resolve the application data directory: {error}"))?;
    if directory.exists() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("Cannot read application data: {error}"))?
        {
            let path = entry
                .map_err(|error| format!("Cannot read application data entry: {error}"))?
                .path();
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .map_err(|error| format!("Cannot clear application data: {error}"))?;
            } else {
                fs::remove_file(&path)
                    .map_err(|error| format!("Cannot clear application data: {error}"))?;
            }
        }
    }

    clear_refresh_token().await?;
    let menu = create_application_menu(&app, &state).map_err(|error| error.to_string())?;
    app.set_menu(menu).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
async fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = url
        .parse::<tauri::Url>()
        .map_err(|error| format!("Cannot open an invalid external URL: {error}"))?;
    if parsed.scheme() != "https" {
        return Err("Only HTTPS URLs can be opened in the system browser.".into());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|error| format!("Cannot open the system browser: {error}"))
}

fn create_application_menu<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &FileAccessState,
) -> tauri::Result<Menu<R>> {
    let command = |id: &str, title: &str, accelerator: Option<&str>| {
        let builder = MenuItemBuilder::with_id(id, title);
        let builder = match accelerator {
            Some(value) => builder.accelerator(value),
            None => builder,
        };
        builder.build(app)
    };
    let open_recent = Submenu::new(app, "Open Recent", true)?;
    let recent_files = state
        .recent_files()
        .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
    if recent_files.is_empty() {
        open_recent.append(
            &MenuItemBuilder::with_id("workspace:no-recent", "No Recent Files")
                .enabled(false)
                .build(app)?,
        )?;
    } else {
        for file in recent_files {
            open_recent.append(&command(
                &format!("{OPEN_RECENT_MENU_PREFIX}{}", file.id),
                &file.name,
                None,
            )?)?;
        }
        open_recent.append(&PredefinedMenuItem::separator(app)?)?;
        open_recent.append(&command("workspace:clear-recent", "Clear Recent", None)?)?;
    }
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &command("workspace:new", "New", Some("CmdOrCtrl+N"))?,
            &command("workspace:open", "Open…", Some("CmdOrCtrl+O"))?,
            &open_recent,
            &PredefinedMenuItem::separator(app)?,
            &command("workspace:save", "Save", Some("CmdOrCtrl+S"))?,
            &command("workspace:save-as", "Save As…", Some("CmdOrCtrl+Shift+S"))?,
            &PredefinedMenuItem::separator(app)?,
            &command("workspace:import", "Import…", None)?,
            &command("workspace:export", "Export…", None)?,
            &PredefinedMenuItem::separator(app)?,
            &command("workspace:close-tab", "Close Tab", Some("CmdOrCtrl+W"))?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[&command(
            "workspace:toggle-viewer",
            "Toggle Viewer",
            Some("CmdOrCtrl+Shift+V"),
        )?],
    )?;
    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[&command("workspace:help", "Treease Help", None)?],
    )?;
    Menu::with_items(app, &[&file, &edit, &view, &help])
}

fn main() {
    if let Err(error) = run() {
        eprintln!("failed to run Treease Desktop Workspace: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), tauri::Error> {
    let builder = tauri::Builder::default()
        .manage(FileAccessState::default())
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            let state = app.state::<FileAccessState>();
            let files = open_explicit_paths(&state, arguments.into_iter().map(PathBuf::from));
            if !files.is_empty() {
                let _ = persist_recent_files_and_refresh_menu(app, &state);
                let _ = app.emit("workspace-files-dropped", files);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build());
    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_wdio::init());
    let app = builder
        .invoke_handler(tauri::generate_handler![
            pick_file,
            read_granted_file,
            save_granted_file,
            save_new_file,
            open_recent_file,
            take_startup_files,
            watch_granted_file,
            unwatch_granted_file,
            list_recent_files,
            clear_recent_files,
            save_workspace_session,
            load_workspace_session,
            reset_application_data,
            open_external_url,
            store_refresh_token,
            has_refresh_token,
            clear_refresh_token,
            refresh_access_token,
        ])
        .setup(|app| {
            load_recent_files(app.handle(), &app.state::<FileAccessState>())
                .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
            let startup_files = open_explicit_paths(
                app.state::<FileAccessState>().inner(),
                std::env::args_os().skip(1).map(PathBuf::from),
            );
            if !startup_files.is_empty() {
                app.state::<FileAccessState>()
                    .startup_files
                    .lock()
                    .map_err(|_| {
                        tauri::Error::Io(std::io::Error::other(
                            "Startup file state is unavailable.",
                        ))
                    })?
                    .extend(startup_files);
                persist_recent_files(app.handle(), app.state::<FileAccessState>().inner())
                    .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
            }
            let menu =
                create_application_menu(app.handle(), app.state::<FileAccessState>().inner())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let command = event.id().as_ref();
            if command.starts_with("workspace:") {
                let _ = app.emit("workspace-command", command);
            }
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event
            else {
                return;
            };
            let state = window.state::<FileAccessState>();
            let files = open_explicit_paths(&state, paths.iter().cloned());
            if !files.is_empty() {
                let _ = persist_recent_files_and_refresh_menu(window.app_handle(), &state);
                let _ = window.emit("workspace-files-dropped", files);
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|app: &tauri::AppHandle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            let _ = app.save_window_state(StateFlags::all());
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        open_explicit_paths, read_json_if_present, supabase_refresh_endpoint,
        validate_workspace_session, write_json_atomically_at, FileAccessState,
    };
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn grants_only_individual_files_and_hides_their_paths() {
        let state = FileAccessState::default();
        let path = std::env::temp_dir().join(format!(
            "treease-desktop-file-access-{}-{}",
            std::process::id(),
            TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "{}\n").expect("create selected file");

        let grant = state.grant(path.clone()).expect("grant selected file");
        assert_eq!(grant.name, path.file_name().unwrap().to_string_lossy());
        assert!(!grant.id.contains(&path.to_string_lossy().to_string()));
        assert_eq!(
            state.path(&grant.id).expect("resolve grant"),
            path.canonicalize().unwrap()
        );
        assert!(state.path("not-granted").is_err());
        assert!(state.grant(std::env::temp_dir()).is_err());

        fs::remove_file(path).expect("remove selected file");
    }

    #[test]
    fn lists_recent_grants_without_exposing_paths_and_can_clear_them() {
        let state = FileAccessState::default();
        let path = std::env::temp_dir().join(format!(
            "treease-desktop-recent-{}-{}",
            std::process::id(),
            TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "{}\n").expect("create selected file");

        let grant = state.grant(path.clone()).expect("grant selected file");
        let recent = state.recent_files().expect("list recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, grant.name);
        assert_ne!(recent[0].id, grant.id);
        assert_eq!(
            state.recent_path(&recent[0].id).expect("resolve recent"),
            path.canonicalize().expect("canonical path")
        );
        state.clear_recent_files().expect("clear recent");
        assert!(state
            .recent_files()
            .expect("list cleared recent")
            .is_empty());

        fs::remove_file(path).expect("remove selected file");
    }

    #[test]
    fn opens_only_explicit_file_paths() {
        let state = FileAccessState::default();
        let path = std::env::temp_dir().join(format!(
            "treease-desktop-startup-{}-{}",
            std::process::id(),
            TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "{}\n").expect("create associated file");

        let files = open_explicit_paths(&state, vec![path.clone(), std::env::temp_dir()]);
        assert_eq!(files.len(), 1);

        fs::remove_file(path).expect("remove associated file");
    }

    #[test]
    fn rejects_workspace_sessions_without_the_supported_shape() {
        assert!(
            validate_workspace_session(&serde_json::json!({ "version": 1, "tabs": [] })).is_ok()
        );
        assert!(
            validate_workspace_session(&serde_json::json!({ "version": 2, "tabs": [] })).is_err()
        );
        assert!(validate_workspace_session(&serde_json::json!({ "version": 1 })).is_err());
    }

    #[test]
    fn persists_a_workspace_recovery_copy_atomically() {
        let directory = std::env::temp_dir().join(format!(
            "treease-desktop-session-{}-{}",
            std::process::id(),
            TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create session directory");
        let session_path = directory.join("session.json");
        let session = serde_json::json!({
            "version": 1,
            "activeTabIndex": 0,
            "tabs": [{ "name": "Draft", "sourceText": "{\\\"saved\\\":false}" }]
        });

        write_json_atomically_at(&session_path, &session).expect("persist recovery copy");
        assert_eq!(
            read_json_if_present(&session_path).expect("read recovery copy"),
            Some(session)
        );

        fs::remove_dir_all(directory).expect("remove session directory");
    }

    #[test]
    fn refreshes_only_the_configured_supabase_origin() {
        assert_eq!(
            supabase_refresh_endpoint("https://project.supabase.co"),
            Ok("https://project.supabase.co/auth/v1/token?grant_type=refresh_token".into())
        );
        assert!(supabase_refresh_endpoint("https://example.com").is_err());
        assert!(supabase_refresh_endpoint("http://project.supabase.co").is_err());
        assert!(supabase_refresh_endpoint("https://project.supabase.co/other").is_err());
    }
}
