//! "Copy URL" / "Copy file name" callbacks.

use std::time::Duration;

use crate::context::AppContext;
use crate::i18n;
use crate::toast::{ToastQueue, push_toast};
use crate::MainWindow;

/// Copy `text` and confirm with a toast. The clipboard call itself runs off the
/// UI thread because it can block on the Windows clipboard.
fn copy_and_confirm(
    ui: &slint::Weak<MainWindow>,
    toast_queue: &ToastQueue,
    text: String,
    message: String,
) {
    let ui = ui.clone();
    let toast_queue = toast_queue.clone();
    tokio::spawn(async move {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&text);
            push_toast(
                &ui,
                &toast_queue,
                message,
                "success",
                Duration::from_secs(3),
            );
        }
    });
}

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let toast_queue = ctx.toast_queue.clone();

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_copy_task_url(move |url| {
            let message = i18n::format_toast_link_copied(store.lock().language()).to_string();
            copy_and_confirm(&ui_weak, &toast_queue, url.to_string(), message);
        });
    }

    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_copy_task_name(move |name| {
            let message = i18n::format_toast_filename_copied(store.lock().language()).to_string();
            copy_and_confirm(&ui_weak, &toast_queue, name.to_string(), message);
        });
    }
}
