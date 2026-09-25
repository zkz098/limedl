use std::rc::Rc;
use std::time::Duration;
use slint::{ComponentHandle, SharedString};

use crate::context::AppContext;
use crate::handlers::new_task::open_new_task_with_payload;
use crate::i18n;
use crate::platform_win;
use crate::protocol;
use crate::ui_sync::restore_and_show_window;

pub fn setup_platform_integration(
    ctx: &AppContext,
    instance_claim: &crate::single_instance::InstanceClaim,
) {
    let main_window = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let new_task_torrent_entries = ctx.new_task_torrent_entries.clone();
    let new_task_torrent_included = ctx.new_task_torrent_included.clone();

    // Cross-platform single-instance activation listener: on macOS/Linux (and on
    // Windows when the secondary can't find our HWND), the secondary process
    // writes its CLI payload over a local socket to wake us up.
    {
        let ui_weak = ui_weak.clone();
        let dispatcher = dispatcher.clone();
        let store = store.clone();
        let store_activate = store.clone();
        let entries_cache = new_task_torrent_entries.clone();
        let included_cache = new_task_torrent_included.clone();
        instance_claim.listen_for_activate(move |payload| {
            let ui_weak_cl = ui_weak.clone();
            let store_for_show = store_activate.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_cl.upgrade() {
                    restore_and_show_window(&ui, Some(&store_for_show.lock()));
                }
            });
            if let Some(p) = payload {
                open_new_task_with_payload(
                    &p,
                    &ui_weak,
                    &dispatcher,
                    &store,
                    &entries_cache,
                    &included_cache,
                );
            }
        });
    }

    let entries_cache = new_task_torrent_entries.clone();
    let included_cache = new_task_torrent_included.clone();

    let ui_weak_drop = ui_weak.clone();
    let dispatcher_drop = dispatcher.clone();
    let store_drop = store.clone();
    let entries_cache_drop = entries_cache.clone();
    let included_cache_drop = included_cache.clone();

    let on_drop = move |files: Vec<String>| {
        let ui_weak = ui_weak_drop.clone();
        let store_for_restore = store_drop.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                restore_and_show_window(&ui, Some(&store_for_restore.lock()));
            }
        });
        if files.len() == 1 {
            open_new_task_with_payload(
                &files[0],
                &ui_weak_drop,
                &dispatcher_drop,
                &store_drop,
                &entries_cache_drop,
                &included_cache_drop,
            );
        } else if !files.is_empty() {
            let torrent_count = files
                .iter()
                .filter(|f| f.to_lowercase().ends_with(".torrent"))
                .count();
            if torrent_count == 1
                && let Some(tf) = files.iter().find(|f| f.to_lowercase().ends_with(".torrent"))
            {
                open_new_task_with_payload(
                    tf,
                    &ui_weak_drop,
                    &dispatcher_drop,
                    &store_drop,
                    &entries_cache_drop,
                    &included_cache_drop,
                );
                return;
            }
            let joined = files.join("\n");
            let count = files.len();
            let ui_weak = ui_weak_drop.clone();
            let store = store_drop.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let lang = store.lock().language();
                    ui.set_new_task_batch_text(SharedString::from(&joined));
                    ui.set_new_task_batch_mode(true);
                    ui.set_new_task_batch_count_text(SharedString::from(
                        i18n::format_batch_count(count, lang),
                    ));
                    ui.set_new_task_batch_submitting(false);
                    ui.set_new_task_batch_status_text(SharedString::default());
                    ui.set_show_new_task_dialog(true);
                }
            });
        }
    };

    let ui_weak_copydata = ui_weak.clone();
    let dispatcher_copydata = dispatcher.clone();
    let store_copydata = store.clone();
    let entries_cache_copydata = entries_cache.clone();
    let included_cache_copydata = included_cache.clone();

    let on_copydata = move |payload: Option<String>| {
        let ui_weak = ui_weak_copydata.clone();
        let dispatcher = dispatcher_copydata.clone();
        let store = store_copydata.clone();
        let entries_cache = entries_cache_copydata.clone();
        let included_cache = included_cache_copydata.clone();
        let store_for_restore = store.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                restore_and_show_window(&ui, Some(&store_for_restore.lock()));
                if let Some(text) = payload {
                    open_new_task_with_payload(
                        &text,
                        &ui_weak,
                        &dispatcher,
                        &store,
                        &entries_cache,
                        &included_cache,
                    );
                }
            }
        });
    };

    let ui_weak_show = ui_weak.clone();
    let store_show = store.clone();
    let on_show = move || {
        let ui_weak = ui_weak_show.clone();
        let store = store_show.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade()
                && !ui.window().is_visible()
            {
                restore_and_show_window(&ui, Some(&store.lock()));
            }
        });
    };

    platform_win::set_callbacks(on_drop, on_copydata, on_show);

    let hook_timer = Rc::new(slint::Timer::default());
    let timer_for_cb = hook_timer.clone();

    if !platform_win::try_install_window_hooks(main_window.window()) {
        hook_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                let Some(ui) = ui_weak.upgrade() else {
                    timer_for_cb.stop();
                    return;
                };
                if platform_win::try_install_window_hooks(ui.window()) {
                    timer_for_cb.stop();
                }
            },
        );
    }

    let _ = protocol::register_protocols();
}
