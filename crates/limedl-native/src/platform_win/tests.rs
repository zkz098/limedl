use super::geometry::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, PLACEMENT_APPLIED};
use super::*;

#[test]
fn os_description_is_non_empty() {
    assert!(!os_description().trim().is_empty());
}

#[test]
fn logon_time_is_sane_when_available() {
    // The shell handle may be unavailable in exotic hosts; validate shape.
    if let Some(secs) = seconds_since_shell_start() {
        assert!(secs < 60 * 60 * 24 * 365, "implausible shell age: {secs}");
    }
    // Only a shell that started "right now" can match a zero window.
    assert!(!launched_at_logon(std::time::Duration::ZERO));
}

#[test]
fn window_geometry_save_and_load_roundtrip() {
    let temp_dir =
        std::env::temp_dir().join(format!("limedl_test_win_geom_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let geom = WindowGeometry {
        x: 250,
        y: 180,
        width: 1280,
        height: 800,
        is_maximized: false,
    };

    save_window_geometry(&temp_dir, &geom);
    let loaded = load_window_geometry(&temp_dir);
    assert_eq!(loaded, Some(geom));

    // Test invalid/too-small dimensions are rejected
    let invalid_geom = WindowGeometry {
        x: 100,
        y: 100,
        width: 200, // too small
        height: 150,
        is_maximized: false,
    };
    save_window_geometry(&temp_dir, &invalid_geom);
    assert_eq!(load_window_geometry(&temp_dir), None);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_clamp_geometry_to_work_area() {
    // 1. In-bounds geometry is preserved
    let normal = WindowGeometry {
        x: 100,
        y: 50,
        width: 1280,
        height: 720,
        is_maximized: false,
    };
    let clamped = clamp_geometry_to_work_area(&normal, 0, 0, 1920, 1040);
    assert_eq!(clamped, normal);

    // 2. Oversized geometry (from 4K/2K display to 1366x768 display) is clamped
    let oversized = WindowGeometry {
        x: 200,
        y: 100,
        width: 2560,
        height: 1440,
        is_maximized: true,
    };
    let clamped = clamp_geometry_to_work_area(&oversized, 0, 0, 1366, 728);
    assert_eq!(clamped.width, 1366);
    assert_eq!(clamped.height, 728);
    assert_eq!(clamped.x, 0);
    assert_eq!(clamped.y, 0);
    assert!(clamped.is_maximized);

    // 3. Right/bottom hanging geometry is shifted inside
    let hanging = WindowGeometry {
        x: 1500,
        y: 800,
        width: 1200,
        height: 700,
        is_maximized: false,
    };
    let clamped = clamp_geometry_to_work_area(&hanging, 0, 0, 1920, 1080);
    assert_eq!(clamped.width, 1200);
    assert_eq!(clamped.height, 700);
    assert_eq!(clamped.x, 720); // 1920 - 1200 = 720
    assert_eq!(clamped.y, 380); // 1080 - 700 = 380

    // 4. Geometry left/above work area is clamped to top-left
    let off_top_left = WindowGeometry {
        x: -200,
        y: -100,
        width: 1000,
        height: 600,
        is_maximized: false,
    };
    let clamped = clamp_geometry_to_work_area(&off_top_left, 0, 0, 1920, 1080);
    assert_eq!(clamped.x, 0);
    assert_eq!(clamped.y, 0);
    assert_eq!(clamped.width, 1000);
    assert_eq!(clamped.height, 600);
}

/// A geometry saved by an earlier run must survive the round trip through
/// both gates: the load-time size filter and the off-screen guard. Only when
/// both accept it is the saved window restored instead of the OS default.
///
/// Uses (0,0), the one origin that resolves to a monitor in every layout,
/// because CI runners are frequently single-display.
#[test]
fn saved_geometry_passes_the_visibility_guard() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    let saved = WindowGeometry {
        x: 0,
        y: 0,
        width: 1024,
        height: 720,
        is_maximized: true,
    };
    save_window_geometry(base, &saved);
    let loaded = load_window_geometry(base).expect("geometry must round trip");
    assert!(is_geometry_visible_on_any_monitor(&loaded));
}

/// A window that lived on a since-disconnected monitor keeps coordinates no
/// current layout can reach; restoring them would place it off-screen.
#[test]
fn off_screen_geometry_is_rejected_by_the_guard() {
    let stale = WindowGeometry {
        x: 40_000,
        y: 30_000,
        width: 1024,
        height: 720,
        is_maximized: false,
    };
    assert!(!is_geometry_visible_on_any_monitor(&stale));
}

/// The load filter drops windows that are too small to be usable, so a
/// degenerate saved size can never be restored. Runs on every platform.
#[test]
fn undersized_geometry_is_rejected_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let too_small = WindowGeometry {
        x: 10,
        y: 10,
        width: MIN_WINDOW_WIDTH - 1,
        height: MIN_WINDOW_HEIGHT,
        is_maximized: false,
    };
    save_window_geometry(dir.path(), &too_small);
    assert_eq!(load_window_geometry(dir.path()), None);
}

#[test]
fn get_base_dir_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let p = temp.path().to_path_buf();
    set_base_dir(p.clone());
    assert_eq!(get_base_dir(), Some(p));
}

#[test]
fn ensure_restored_window_placement_is_idempotent() {
    use slint::ComponentHandle;

    i_slint_backend_testing::init_no_event_loop();
    let ui = crate::MainWindow::new().unwrap();
    reset_placement_applied_for_test();
    assert!(!PLACEMENT_APPLIED.load(Ordering::SeqCst));
    PLACEMENT_APPLIED.store(true, Ordering::SeqCst);
    let temp = tempfile::tempdir().unwrap();
    assert!(ensure_restored_window_placement(
        ui.window(),
        temp.path(),
        false
    ));
    reset_placement_applied_for_test();
}

#[test]
fn test_trim_working_set() {
    trim_working_set();
}

#[test]
fn test_window_visibility_channel() {
    let mut rx = subscribe_window_visibility();
    set_window_visible(false);
    assert!(!is_window_visible());
    assert!(!*rx.borrow_and_update());

    set_window_visible(true);
    assert!(is_window_visible());
    assert!(*rx.borrow_and_update());
}
