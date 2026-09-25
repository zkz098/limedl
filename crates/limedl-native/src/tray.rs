use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::TrayIcon;

use crate::i18n::{self, Language};

/// Speed applied when the user enables the tray "speed limit" shortcut.
/// Kept at the historical 1 MiB/s default.
pub const TRAY_SPEED_LIMIT_BPS: u64 = 1_048_576;

pub fn build_tray_menu(lang: Language, speed_limit_active: bool) -> Menu {
    let t = i18n::get_tray_strings(lang);
    let tray_menu = Menu::new();
    let menu_show = MenuItem::with_id("show", t.show_window, true, None);
    let sep1 = PredefinedMenuItem::separator();
    let menu_pause_all = MenuItem::with_id("pause_all", t.pause_all, true, None);
    let menu_resume_all = MenuItem::with_id("resume_all", t.resume_all, true, None);
    let menu_speed_limit = CheckMenuItem::with_id(
        "speed_limit",
        t.speed_limit_toggle,
        true,
        speed_limit_active,
        None,
    );
    let menu_game_mode = MenuItem::with_id("game_mode", t.game_mode_toggle, true, None);
    let menu_open_dir = MenuItem::with_id("open_dir", t.open_download_dir, true, None);
    let sep2 = PredefinedMenuItem::separator();
    let menu_quit = MenuItem::with_id("quit", t.quit, true, None);

    let _ = tray_menu.append_items(&[
        &menu_show,
        &sep1,
        &menu_pause_all,
        &menu_resume_all,
        &menu_speed_limit,
        &menu_game_mode,
        &menu_open_dir,
        &sep2,
        &menu_quit,
    ]);
    tray_menu
}

/// Explain a tray-icon construction failure.
pub fn tray_init_failure_message(err: &tray_icon::Error) -> String {
    #[cfg(target_os = "linux")]
    {
        format!(
            "failed to create the system tray icon: {err}\n\
             limedl requires a system tray. Install a StatusNotifier/appindicator host, e.g.\n\
             \x20 Debian/Ubuntu: sudo apt install libayatana-appindicator3-1 gnome-shell-extension-appindicator\n\
             \x20 Fedora:        sudo dnf install libappindicator-gtk3\n\
             \x20 Arch:          sudo pacman -S libappindicator-gtk3\n\
             On GNOME also enable the AppIndicator extension, then start limedl again."
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        format!("failed to create the system tray icon: {err}")
    }
}

thread_local! {
    pub static TRAY_INSTANCE: std::cell::RefCell<Option<TrayIcon>> = const { std::cell::RefCell::new(None) };
}

/// Update the system tray menu and tooltip on the UI thread without periodic polling timers.
pub fn update_tray_menu_and_tooltip(lang: Language, speed_limit_active: bool) {
    TRAY_INSTANCE.with(|cell| {
        if let Some(tray) = cell.borrow_mut().as_mut() {
            tray.set_menu(Some(Box::new(build_tray_menu(lang, speed_limit_active))));
            let _ = tray.set_tooltip(Some(i18n::get_tray_strings(lang).tooltip));
        }
    });
}

pub fn create_default_tray_icon() -> tray_icon::Icon {
    const ICON_BYTES: &[u8] = include_bytes!("../ui/assets/32x32.png");
    if let Ok(dyn_img) = image::load_from_memory_with_format(ICON_BYTES, image::ImageFormat::Png) {
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba.into_raw(), width, height) {
            return icon;
        }
    }

    // Fallback: 32x32 RGBA icon
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let is_l = ((8..=12).contains(&x) && (6..=26).contains(&y))
                || ((8..=24).contains(&x) && (22..=26).contains(&y));
            if is_l {
                rgba.extend_from_slice(&[132, 204, 22, 255]);
            } else {
                rgba.extend_from_slice(&[24, 27, 31, 230]);
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).unwrap()
}
