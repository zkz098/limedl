//! The toast queue's own contract: one entry per notification, dismissed
//! individually.
//!
//! The *rendering* of a toast happens through `invoke_from_event_loop`, which
//! `with_ui` never drains, so the row is not in the element tree and cannot be
//! clicked. These tests therefore push through `push_toast` (the same function
//! every handler uses) and dismiss through the window callback the close button
//! invokes — the id-based removal is the part that decides which toast goes.
//! The anti-flood rule for repeated warnings is a pure unit test in
//! `src/toast.rs`.

use std::time::Duration;

use crate::toast::push_toast;

use slint::ComponentHandle;

use super::with_ui;

#[test]
fn toasts_are_pushed_in_order_and_dismissed_individually() {
    with_ui(|ui| {
        let ui_weak = ui.window.as_weak();
        push_toast(
            &ui_weak,
            &ui.ctx.toast_queue,
            "first message".to_string(),
            "success",
            Duration::from_secs(4),
        );
        push_toast(
            &ui_weak,
            &ui.ctx.toast_queue,
            "second message".to_string(),
            "error",
            Duration::from_secs(6),
        );

        assert_eq!(
            ui.toasts(),
            vec![
                ("success".to_string(), "first message".to_string()),
                ("error".to_string(), "second message".to_string()),
            ]
        );

        let ids = ui.toast_ids();
        assert_eq!(
            ids.len(),
            2,
            "each toast has an id the close button can name"
        );
        assert_ne!(
            ids[0], ids[1],
            "ids must be unique or the wrong one is dismissed"
        );

        // Dismiss through the callback the close button invokes: ids are
        // monotonic counters, so this also pins that the queue is not indexed by
        // position.
        ui.dismiss_toast(ids[0]);
        ui.assert_toast("error", "second message");

        ui.dismiss_toast(ids[1]);
        assert!(ui.toasts().is_empty());

        // Dismissing an unknown id (a stale close button) is a no-op.
        ui.dismiss_toast(99_999);
        assert!(ui.toasts().is_empty());
    });
}
