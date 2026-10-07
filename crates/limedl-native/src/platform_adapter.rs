use crate::context::AppContext;
use crate::handlers::new_task::open_new_task_with_payload;
use crate::protocol;
use crate::ui_sync::restore_and_show_window;
#[cfg(windows)]
use crate::MainWindow;

pub fn setup_platform_integration(
    ctx: &AppContext,
    instance_claim: &crate::single_instance::InstanceClaim,
) {
    // Cross-platform single-instance activation listener: on macOS/Linux (and on
    // Windows when the secondary can't find our HWND), the secondary process
    // writes its CLI payload over a local socket to wake us up.
    {
        let ui_weak = ctx.ui_weak.clone();
        let dispatcher = ctx.dispatcher.clone();
        let store = ctx.store.clone();
        let store_activate = store.clone();
        let entries_cache = ctx.new_task_torrent_entries.clone();
        let included_cache = ctx.new_task_torrent_included.clone();
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

    #[cfg(windows)]
    setup_windows_hooks(ctx);

    let _ = protocol::register_protocols();
}

#[cfg(windows)]
fn setup_windows_hooks(ctx: &AppContext) {
    use std::rc::Rc;
    use std::time::Duration;
    use slint::ComponentHandle;
    use crate::platform_win;

    let main_window = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let entries_cache = ctx.new_task_torrent_entries.clone();
    let included_cache = ctx.new_task_torrent_included.clone();

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
        open_dropped_files(
            &files,
            &ui_weak_drop,
            &dispatcher_drop,
            &store_drop,
            &entries_cache_drop,
            &included_cache_drop,
        );
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
}

/// Route a multi-file drop: one file opens directly, a sole `.torrent` or
/// `.metalink`/`.meta4` among many opens that file, otherwise the whole list
/// becomes a batch.
#[cfg(windows)]
fn open_dropped_files(
    files: &[String],
    ui_weak: &slint::Weak<MainWindow>,
    dispatcher: &std::sync::Arc<limedl_core::dispatcher::Dispatcher>,
    store: &std::sync::Arc<parking_lot::Mutex<crate::bridge::TaskStore>>,
    entries_cache: &std::sync::Arc<
        parking_lot::Mutex<Vec<limedl_core::types::TorrentFileEntry>>,
    >,
    included_cache: &std::sync::Arc<parking_lot::Mutex<Vec<bool>>>,
) {
    use slint::SharedString;

    if files.len() == 1 {
        open_new_task_with_payload(
            &files[0],
            ui_weak,
            dispatcher,
            store,
            entries_cache,
            included_cache,
        );
        return;
    }
    if files.is_empty() {
        return;
    }

    if let Some(tf) = sole_file_with_suffix(files, &[".torrent"]) {
        open_new_task_with_payload(
            &tf,
            ui_weak,
            dispatcher,
            store,
            entries_cache,
            included_cache,
        );
        return;
    }
    if let Some(mf) = sole_file_with_suffix(files, &[".metalink", ".meta4"]) {
        open_new_task_with_payload(
            &mf,
            ui_weak,
            dispatcher,
            store,
            entries_cache,
            included_cache,
        );
        return;
    }

    let joined = files.join("\n");
    let count = files.len();
    let ui_for_ui = (*ui_weak).clone();
    let store_for_ui = (*store).clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_for_ui.upgrade() {
            let lang = store_for_ui.lock().language();
            ui.set_new_task_batch_text(SharedString::from(&joined));
            ui.set_new_task_batch_mode(true);
            ui.set_new_task_batch_count_text(SharedString::from(
                crate::i18n::format_batch_count(count, lang),
            ));
            ui.set_new_task_batch_submitting(false);
            ui.set_new_task_batch_status_text(SharedString::default());
            ui.set_show_new_task_dialog(true);
        }
    });
}

/// Return the file when exactly one entry matches one of `suffixes`.
#[cfg(windows)]
fn sole_file_with_suffix(files: &[String], suffixes: &[&str]) -> Option<String> {
    let matched: Vec<&String> = files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            suffixes.iter().any(|suffix| lower.ends_with(suffix))
        })
        .collect();
    match matched.as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    }
}

