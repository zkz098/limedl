//! The toast queue's own contract: one entry per notification, dismissed
//! individually, and retired through an exit animation.
//!
//! The *rendering* of a toast happens through `invoke_from_event_loop`, which
//! `with_ui` never drains, so the row is not in the element tree and cannot be
//! clicked. These tests therefore push through `push_toast` (the same function
//! every handler uses) and dismiss through the window callback the close button
//! invokes — the id-based removal is the part that decides which toast goes.
//! The anti-flood rule for repeated warnings is a pure unit test in
//! `src/toast.rs`.

use std::time::Duration;

use slint::Model;

use crate::toast::{apply_toasts, push_toast};
use crate::ToastItem;

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
        // position. The entry is marked `leaving` rather than dropped so the
        // card can slide out; it is no longer "pending".
        ui.dismiss_toast(ids[0]);
        assert!(
            !ui.ctx
                .toast_queue
                .lock()
                .iter()
                .any(|entry| entry.id == ids[0] as usize && !entry.leaving),
            "the dismissed toast must be retired, not still pending"
        );
        assert!(
            ui.ctx
                .toast_queue
                .lock()
                .iter()
                .any(|entry| entry.id == ids[0] as usize),
            "the dismissed toast stays in the queue for its exit animation"
        );
        ui.assert_toast("error", "second message");

        ui.dismiss_toast(ids[1]);
        assert!(ui.toasts().is_empty());

        // Dismissing an unknown id (a stale close button) is a no-op.
        ui.dismiss_toast(99_999);
        assert!(ui.toasts().is_empty());
    });
}

/// The model diff behind `sync_toasts`: rows whose id survives are updated in
/// place, so dismissing one toast does not recreate (and re-animate) the rest.
#[test]
fn apply_toasts_reuses_surviving_rows() {
    with_ui(|ui| {
        let item = |id: i32, message: &str, leaving: bool| ToastItem {
            id,
            message: message.into(),
            kind: "info".into(),
            leaving,
        };

        apply_toasts(
            &ui.window,
            vec![item(1, "first", false), item(2, "second", false)],
        );
        let model = ui.window.get_toasts();
        assert_eq!(model.row_count(), 2);
        // Force the toast stack to lay out: the card's height comes from its
        // content while the layout slot mirrors it, so a binding loop here would
        // only show up once the tree is queried.
        assert!(
            ui.has("ToastStack::toast_close"),
            "the toast card must lay out once the model has rows"
        );

        // Mark the first toast as leaving: both rows must remain, and the
        // untouched second row must keep its identity and content.
        apply_toasts(
            &ui.window,
            vec![item(1, "first", true), item(2, "second", false)],
        );
        assert_eq!(model.row_count(), 2);
        assert!(model.row_data(0).unwrap().leaving);
        assert_eq!(model.row_data(1).unwrap().id, 2);
        assert!(!model.row_data(1).unwrap().leaving);

        // Once the exit timer drops the first, only the survivor is left.
        apply_toasts(&ui.window, vec![item(2, "second", false)]);
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.row_data(0).unwrap().id, 2);
    });
}
