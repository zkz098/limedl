use slint::{CloseRequestResponse, ComponentHandle};
use limedl_core::types::CloseBehavior;

use crate::context::AppContext;
use crate::platform_win;
use crate::toast::dismiss_toast;

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let toast_queue = ctx.toast_queue.clone();
    let current_settings = ctx.current_settings.clone();
    let base_dir = ctx.base_dir.clone();

    // In-app toast dismiss (close button)
    {
        let ui_weak = ui_weak.clone();
        let toast_queue = toast_queue.clone();
        main_window.on_dismiss_toast(move |id| {
            dismiss_toast(&ui_weak, &toast_queue, id);
        });
    }

    // Window close behavior
    {
        let ui_weak = ui_weak.clone();
        let current_settings_clone = current_settings.clone();
        let base_dir_for_close = base_dir.clone();
        main_window.window().on_close_requested(move || {
            if let Some(ui) = ui_weak.upgrade() {
                platform_win::save_current_window_geometry(ui.window(), &base_dir_for_close);
            }
            let minimize_to_tray = matches!(
                current_settings_clone.lock().appearance.close_behavior,
                CloseBehavior::MinimizeToTray
            );
            if minimize_to_tray {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.hide();
                    platform_win::hide_window(ui.window());
                }
                tracing::info!("窗口已最小化到托盘");
            } else {
                tracing::info!("窗口关闭，退出应用");
                let _ = slint::quit_event_loop();
            }
            CloseRequestResponse::HideWindow
        });
    }
}
