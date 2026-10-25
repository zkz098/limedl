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
             limedl serves the tray as a StatusNotifierItem over the session D-Bus, so it needs\n\
             a running D-Bus session and a StatusNotifier host:\n\
             \x20 GNOME: install and enable the AppIndicator shell extension\n\
             \x20 KDE/XFCE/waybar/swaybar: the host ships with the desktop or bar\n\
             No GTK/appindicator package is involved anymore. If only the host is missing,\n\
             limedl keeps running and the icon appears once the host shows up."
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
    if let Some((rgba, width, height)) = embedded_png_rgba(ICON_BYTES)
        && let Ok(icon) = tray_icon::Icon::from_rgba(rgba, width, height)
    {
        return icon;
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

/// Decode the bundled icon into the RGBA8 buffer `tray-icon` wants.
///
/// `normalize_to_color8` (expand palette/low bit depths, strip 16-bit samples)
/// plus `ALPHA` (expand a `tRNS` chunk, or an opaque channel) covers every shape
/// an icon asset realistically takes. Grayscale output is *not* handled: it falls
/// through to the drawn fallback above rather than being guessed at.
fn embedded_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let pixels = buffer.get(..frame.buffer_size())?;

    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        // A trailing partial pixel cannot exist in an 8-bit RGB frame; if it did,
        // dropping it is better than panicking on a corrupt asset.
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], u8::MAX])
            .collect(),
        _ => return None,
    };

    Some((rgba, frame.width, frame.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled PNG has to decode through `png` the same way it decoded through
    /// `image`. This is silent when it breaks — the drawn fallback below still
    /// yields a usable (if wrong) tray icon — so assert on the pixels instead.
    #[test]
    fn bundled_png_decodes_to_32x32_rgba() {
        const ICON_BYTES: &[u8] = include_bytes!("../ui/assets/32x32.png");
        let (rgba, width, height) = embedded_png_rgba(ICON_BYTES).expect("bundled PNG decodes");

        assert_eq!((width, height), (32, 32));
        assert_eq!(rgba.len(), 32 * 32 * 4);
        // Artwork-independent sanity: the icon is neither fully transparent nor
        // fully black, which is what a wrong stride/color-type read produces.
        let (pixels, remainder) = rgba.as_chunks::<4>();
        assert!(remainder.is_empty(), "ragged RGBA buffer");
        assert!(pixels.iter().any(|px| px[3] == 255), "no opaque pixel");
        assert!(
            pixels.iter().any(|px| px[..3].iter().any(|&c| c != 0)),
            "fully black icon"
        );
    }
}
