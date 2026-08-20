use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

const PET_WINDOW_LABEL: &str = "pet";
const PET_BASE_SIZE: f64 = 160.0;
const PET_MIN_SIZE: f64 = 80.0;
const PET_MAX_SIZE: f64 = 500.0;

/// Ensure the pet window exists. Creates it lazily if missing.
/// Caller should check `settings.pet.enabled` before calling.
pub fn ensure_pet_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        return Ok(window);
    }

    // Read pet settings for initial size/position
    let pet = {
        let state = app.state::<limedl_core::manager::AppState>();
        state.settings.read().pet.clone()
    };

    let scale = pet.scale.clamp(0.5, 2.0);
    let size = (PET_BASE_SIZE * scale).clamp(PET_MIN_SIZE, PET_MAX_SIZE);

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        PET_WINDOW_LABEL,
        tauri::WebviewUrl::App("pet.html".into()),
    )
    .title("pet")
    .inner_size(size, size)
    .min_inner_size(PET_MIN_SIZE, PET_MIN_SIZE)
    .max_inner_size(PET_MAX_SIZE, PET_MAX_SIZE)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .accept_first_mouse(true);

    #[cfg(target_os = "macos")]
    let builder = builder.visible_on_all_workspaces(true);

    let window = builder.build().map_err(|e| e.to_string())?;

    // Apply initial position if saved, otherwise bottom-right of primary monitor
    // Saved x/y are physical pixels (from outerPosition), so use PhysicalPosition
    let scale = window.scale_factor().unwrap_or(1.0);
    let phys_w = (size * scale) as i32;
    let phys_h = (size * scale) as i32;
    let positioned = if let (Some(x), Some(y)) = (pet.x, pet.y) {
        // Validate saved position is fully visible on a monitor; if not, fallback to bottom-right
        let on_monitor = window
            .available_monitors()
            .ok()
            .map(|mons| {
                mons.iter().any(|m| {
                    let pos = m.position();
                    let sz = m.size();
                    x >= pos.x
                        && x + phys_w <= pos.x + sz.width as i32
                        && y >= pos.y
                        && y + phys_h <= pos.y + sz.height as i32
                })
            })
            .unwrap_or(true);
        if on_monitor {
            let _ =
                window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            true
        } else {
            false
        }
    } else {
        false
    };
    if !positioned && let Ok(Some(monitor)) = window.primary_monitor() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            let margin_x = (20.0 * scale) as i32;
            let margin_y = (60.0 * scale) as i32;
            let x = monitor_pos.x + monitor_size.width as i32 - phys_w - margin_x;
            let y = monitor_pos.y + monitor_size.height as i32 - phys_h - margin_y;
            let _ = window
                .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        }

    // Show after positioning to avoid flash at wrong location
    let _ = window.show();

    Ok(window)
}

pub fn destroy_pet_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window.destroy().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_show(app: AppHandle) -> Result<(), String> {
    let window = ensure_pet_window(&app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn pet_hide(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_close(app: AppHandle) -> Result<(), String> {
    destroy_pet_window(&app)
}

#[tauri::command]
pub async fn pet_set_scale(app: AppHandle, scale: f64) -> Result<(), String> {
    let scale = scale.clamp(0.5, 2.0);
    let size = (PET_BASE_SIZE * scale).clamp(PET_MIN_SIZE, PET_MAX_SIZE);
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: size,
                height: size,
            }))
            .map_err(|e| e.to_string())?;
    }
    // Persist scale
    {
        let state = app.state::<limedl_core::manager::AppState>();
        let mut settings = state.settings.write();
        settings.pet.scale = scale;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_set_ignore_cursor_events(
    app: AppHandle,
    ignore: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window
            .set_ignore_cursor_events(ignore)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_start_drag(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL) {
        window.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_update_position(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    // Validate: ensure position is on some monitor, otherwise ignore
    if let Some(window) = app.get_webview_window(PET_WINDOW_LABEL)
        && let Ok(monitors) = window.available_monitors()
    {
        let on_monitor = monitors.iter().any(|m| {
            let pos = m.position();
            let size = m.size();
            x >= pos.x
                && y >= pos.y
                && x < pos.x + size.width as i32
                && y < pos.y + size.height as i32
        });
        // Still persist even if off-monitor — user may have unplugged display
        let _ = on_monitor;
    }

    {
        let state = app.state::<limedl_core::manager::AppState>();
        let mut settings = state.settings.write();
        settings.pet.x = Some(x);
        settings.pet.y = Some(y);
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_set_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    {
        let state = app.state::<limedl_core::manager::AppState>();
        let mut settings = state.settings.write();
        settings.pet.enabled = enabled;
    }
    if enabled {
        let _ = ensure_pet_window(&app)?;
    } else {
        let _ = destroy_pet_window(&app);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetMenuState {
    pub has_active: bool,
    pub speed_limit_active: bool,
    pub game_mode: bool,
    pub main_visible: bool,
}

#[tauri::command]
pub async fn pet_get_menu_state(app: AppHandle) -> Result<PetMenuState, String> {
    use limedl_core::manager::DownloadManager;
    use limedl_core::types::DownloadState;

    let state = app.state::<limedl_core::manager::AppState>();
    let has_active = if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        dm.list()
            .await
            .map(|list| list.iter().any(|s| matches!(s.state, DownloadState::Downloading)))
            .unwrap_or(false)
    } else {
        false
    };
    let settings = state.settings.read();
    let speed_limit_active = settings.global_speed_limit_bps > 0;
    let game_mode = if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        dm.game_mode()
    } else {
        false
    };
    drop(settings);
    let main_visible = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    Ok(PetMenuState {
        has_active,
        speed_limit_active,
        game_mode,
        main_visible,
    })
}

#[tauri::command]
pub async fn pet_toggle_pause_all(app: AppHandle) -> Result<(), String> {
    use limedl_core::types::{DownloadState, TaskId};
    use limedl_core::Dispatcher;

    let state = app.state::<limedl_core::manager::AppState>();
    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let list = dispatcher.list().await.map_err(|e| e.to_string())?;
    let has_active = list
        .iter()
        .any(|s| matches!(s.state, DownloadState::Downloading));
    for s in &list {
        let Ok(task_id) = TaskId::from_wire_string(&s.id) else {
            continue;
        };
        if has_active && matches!(s.state, DownloadState::Downloading) {
            let _ = dispatcher.pause(&task_id).await;
        } else if !has_active && matches!(s.state, DownloadState::Paused) {
            let _ = dispatcher.resume(&task_id).await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_toggle_speed_limit(app: AppHandle) -> Result<(), String> {
    use limedl_core::manager::DownloadManager;
    let state = app.state::<limedl_core::manager::AppState>();
    {
        let mut settings = state.settings.write();
        if settings.global_speed_limit_bps > 0 {
            settings.global_speed_limit_bps = 0;
        } else {
            settings.global_speed_limit_bps = 1_048_576;
        }
    }
    if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        let s = state.settings.read().clone();
        let _ = dm.apply_settings(s).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_toggle_game_mode(app: AppHandle) -> Result<(), String> {
    use limedl_core::manager::DownloadManager;
    let state = app.state::<limedl_core::manager::AppState>();
    if let Some(dm) = state.registry.get_typed::<DownloadManager>() {
        let current = dm.game_mode();
        dm.set_game_mode(!current);
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_open_download_dir(app: AppHandle) -> Result<(), String> {
    let state = app.state::<limedl_core::manager::AppState>();
    let dir = state
        .settings
        .read()
        .download
        .default_download_dir
        .clone();
    if !dir.is_empty() {
        tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_show_main(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    // Restore pet if it was hidden with main (keepAlive=false case)
    let pet_enabled = app
        .state::<limedl_core::manager::AppState>()
        .settings
        .read()
        .pet
        .enabled;
    if pet_enabled {
        if let Some(pet) = app.get_webview_window("pet") {
            let _ = pet.show();
        } else {
            let _ = ensure_pet_window(&app);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pet_open_settings(app: AppHandle) -> Result<(), String> {
    // Ensure main window is visible first
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Main window not yet created (unlikely), try to show via pet_show_main
        let _ = pet_show_main(app.clone()).await;
    }
    // Notify frontend to open settings with pet tab
    let _ = app.emit("open-settings", serde_json::json!({ "tab": "pet" }));
    // Also emit generic open-settings for backward compat
    let _ = app.emit("open-settings-pet", ());
    Ok(())
}

#[tauri::command]
pub async fn pet_quit(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
