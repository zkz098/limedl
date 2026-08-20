use tauri::{AppHandle, Manager, WebviewWindow};

const PET_WINDOW_LABEL: &str = "pet";
const PET_BASE_SIZE: f64 = 160.0;
const PET_MIN_SIZE: f64 = 80.0;
const PET_MAX_SIZE: f64 = 320.0;

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
    if let (Some(x), Some(y)) = (pet.x, pet.y) {
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
    } else if let Ok(Some(monitor)) = window.primary_monitor() {
        let monitor_size = monitor.size();
        let monitor_pos = monitor.position();
        // Fallback: bottom-right with 20px margin
        let x = monitor_pos.x + monitor_size.width as i32 - size as i32 - 20;
        let y = monitor_pos.y + monitor_size.height as i32 - size as i32 - 60;
        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
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
            .set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: size as u32,
                height: size as u32,
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
