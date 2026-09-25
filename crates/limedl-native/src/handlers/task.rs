use std::time::Duration;
use slint::ComponentHandle;
use limedl_core::types::{DownloadState, TaskId};

use crate::context::AppContext;
use crate::bridge::{SortField, str_to_priority};
use crate::i18n;
use crate::task_ops::handle_task_double_click;
use crate::toast::push_toast;
use crate::ui_sync::{persist_sort_preference, refresh_ui};

pub fn register(ctx: &AppContext) {
    let main_window = &ctx.ui;
    let dispatcher = ctx.dispatcher.clone();
    let store = ctx.store.clone();
    let current_settings = ctx.current_settings.clone();
    let toast_queue = ctx.toast_queue.clone();
    let active_inspector_id = ctx.active_inspector_id.clone();

    // Select Category
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_select_category(move |cat| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_category(cat);
                ui.set_active_category(cat);
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Search changed
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_search_changed(move |query| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.set_search_query(query.to_string());
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Sort field & order
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_set_sort_field(move |field_idx| {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let (field, asc) = {
                    let mut store = store_clone.lock();
                    store.set_sort_field(SortField::from(field_idx));
                    refresh_ui(&ui, &store);
                    (store.sort_field(), store.sort_asc())
                };
                persist_sort_preference(&dispatcher, &current_settings_clone, field, asc);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let dispatcher = dispatcher.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_toggle_sort_asc(move || {
            let dispatcher = dispatcher.clone();
            let current_settings_clone = current_settings_clone.clone();
            if let Some(ui) = ui_weak.upgrade() {
                let (field, asc) = {
                    let mut store = store_clone.lock();
                    store.toggle_sort_order();
                    refresh_ui(&ui, &store);
                    (store.sort_field(), store.sort_asc())
                };
                persist_sort_preference(&dispatcher, &current_settings_clone, field, asc);
            }
        });
    }

    // Phase 2: Multi-selection
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_toggle_select_task(move |id_str| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.toggle_select(&id_str);
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_range_select_task(move |id_str| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.select_range(&id_str);
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_select_all(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.select_all();
                refresh_ui(&ui, &store);
            }
        });
    }

    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_clear_selection(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                store.clear_selection();
                refresh_ui(&ui, &store);
            }
        });
    }

    // Phase 2: Batch actions
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_batch_pause(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let store = store_clone.lock();
                    store.selected_ids()
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        let _ = dispatcher.pause(&task_id).await;
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_batch_resume(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let ids = {
                    let store = store_clone.lock();
                    store.selected_ids()
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        let _ = dispatcher.resume(&task_id).await;
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_batch_remove(move |delete_files| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let active_inspector_id = active_inspector_id_clone.clone();

            let ids = {
                let mut store = store_clone.lock();
                let ids = store.selected_ids();
                store.clear_selection();
                for id in &ids {
                    store.remove(id);
                }
                ids
            };

            if ids.is_empty() {
                return;
            }

            if let Some(ui) = ui_weak.upgrade() {
                let store = store_clone.lock();
                refresh_ui(&ui, &store);
                if let Some(ref current_id) = *active_inspector_id.lock()
                    && ids.contains(current_id)
                {
                    ui.set_show_inspector(false);
                }
            }

            tokio::spawn(async move {
                for id in &ids {
                    if let Ok(task_id) = TaskId::from_wire_string(id) {
                        if delete_files {
                            let _ = dispatcher.purge(&task_id).await;
                        } else {
                            let _ = dispatcher.remove(&task_id).await;
                        }
                    }
                }

                if let Ok(list) = dispatcher.list().await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut store = store_clone.lock();
                            store.replace_all(list);
                            refresh_ui(&ui, &store);
                        }
                    });
                }
            });
        });
    }

    // View mode toggle (Cards <-> Table)
    {
        let ui_weak = main_window.as_weak();
        main_window.on_toggle_view_mode(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let current = ui.get_view_mode();
                ui.set_view_mode(if current == 0 { 1 } else { 0 });
            }
        });
    }

    // Table column header sort clicked
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        main_window.on_table_sort_clicked(move |col_idx| {
            if let Some(ui) = ui_weak.upgrade() {
                let mut store = store_clone.lock();
                let target_field = SortField::from(col_idx);
                if store.sort_field() == target_field as i32 {
                    store.toggle_sort_order();
                } else {
                    store.set_sort_field(target_field);
                }
                refresh_ui(&ui, &store);
            }
        });
    }

    // Desktop keyboard shortcuts: Space (toggle pause/resume)
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_hotkey_space(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();

            tokio::spawn(async move {
                let (ids, any_downloading) = {
                    let store = store_clone.lock();
                    let sel = store.selected_ids();
                    if !sel.is_empty() {
                        let downloading = store.filtered_items().iter().any(|item| {
                            sel.contains(&item.id.to_string()) && item.can_pause
                        });
                        (sel, downloading)
                    } else {
                        let all_ids: Vec<String> = store
                            .filtered_items()
                            .iter()
                            .map(|it| it.id.to_string())
                            .collect();
                        let (_, downloading_count, _, _, _) = store.counts();
                        (all_ids, downloading_count > 0)
                    }
                };

                for id in ids {
                    if let Ok(task_id) = TaskId::from_wire_string(&id) {
                        if any_downloading {
                            let _ = dispatcher.pause(&task_id).await;
                        } else {
                            let _ = dispatcher.resume(&task_id).await;
                        }
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let store = store_clone.lock();
                        refresh_ui(&ui, &store);
                    }
                });
            });
        });
    }

    // Desktop keyboard shortcuts: Delete (delete selected tasks, shift: purge files)
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_hotkey_delete(move |delete_files| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let active_inspector_id = active_inspector_id_clone.clone();

            let ids = {
                let mut store = store_clone.lock();
                let ids = store.selected_ids();
                store.clear_selection();
                for id in &ids {
                    store.remove(id);
                }
                ids
            };

            if ids.is_empty() {
                return;
            }

            if let Some(ui) = ui_weak.upgrade() {
                let store = store_clone.lock();
                refresh_ui(&ui, &store);
                if let Some(ref current_id) = *active_inspector_id.lock()
                    && ids.contains(current_id)
                {
                    ui.set_show_inspector(false);
                }
            }

            tokio::spawn(async move {
                for id in &ids {
                    if let Ok(task_id) = TaskId::from_wire_string(id) {
                        if delete_files {
                            let _ = dispatcher.purge(&task_id).await;
                        } else {
                            let _ = dispatcher.remove(&task_id).await;
                        }
                    }
                }

                if let Ok(list) = dispatcher.list().await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut store = store_clone.lock();
                            store.replace_all(list);
                            refresh_ui(&ui, &store);
                        }
                    });
                }
            });
        });
    }

    // Phase 3: Task Inspector callbacks
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let dispatcher_clone = dispatcher.clone();

        main_window.on_set_task_priority(move |id_str, priority_code| {
            let dispatcher = dispatcher_clone.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let id_str = id_str.to_string();
            let priority_code = priority_code.to_string();
            tokio::spawn(async move {
                let lang = store_clone.lock().language();
                let priority = str_to_priority(&priority_code);
                let file_name = store_clone
                    .lock()
                    .get_summary(&id_str)
                    .map(|s| s.file_name)
                    .unwrap_or_default();
                let Ok(task_id) = TaskId::from_wire_string(&id_str) else {
                    tracing::warn!("优先级设置失败: 无法解析任务 ID {id_str}");
                    return;
                };
                match dispatcher.set_priority(&task_id, priority).await {
                    Ok(()) => push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_priority_set(&file_name, priority, lang),
                        "info",
                        Duration::from_secs(4),
                    ),
                    Err(err) => {
                        tracing::error!("优先级设置失败: {err:#}");
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_priority_failed(&format!("{err:#}"), lang),
                            "error",
                            Duration::from_secs(6),
                        );
                    }
                }
            });
        });
    }

    // Pause Task
    {
        let dispatcher = dispatcher.clone();
        main_window.on_pause_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.pause(&task_id).await
                {
                    tracing::error!("暂停任务失败: {err}");
                }
            });
        });
    }

    // Resume Task
    {
        let dispatcher = dispatcher.clone();
        main_window.on_resume_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.resume(&task_id).await
                {
                    tracing::error!("恢复任务失败: {err}");
                }
            });
        });
    }

    // Open in Explorer
    {
        let dispatcher = dispatcher.clone();
        main_window.on_open_task_explorer(move |id_str| {
            let dispatcher = dispatcher.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.open_in_explorer(&task_id).await
                {
                    tracing::error!("打开任务文件目录失败: {err}");
                }
            });
        });
    }

    // Double-click behavior (Settings → General: double_click on_completed /
    // on_uncompleted). Mirrors the web client: completed tasks open the file /
    // explorer / download dir; uncompleted tasks toggle pause/resume.
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let current_settings_clone = current_settings.clone();
        main_window.on_task_double_clicked(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let current_settings_clone = current_settings_clone.clone();
            let id_str = id_str.to_string();
            tokio::spawn(async move {
                if let Err(err) = handle_task_double_click(
                    &dispatcher,
                    &store_clone,
                    &current_settings_clone,
                    &id_str,
                )
                .await
                {
                    tracing::error!("双击任务操作失败: {err:#}");
                }
            });
        });
    }

    // Remove Task
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_remove_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let active_inspector_id = active_inspector_id_clone.clone();
            let id_str = id_str.to_string();

            // 1. Immediate optimistic UI removal
            {
                let mut store = store_clone.lock();
                store.remove(&id_str);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let store = store_clone.lock();
                refresh_ui(&ui, &store);
                if let Some(ref current_id) = *active_inspector_id.lock()
                    && current_id == &id_str
                {
                    ui.set_show_inspector(false);
                }
            }

            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id_str)
                    && let Err(err) = dispatcher.remove(&task_id).await
                {
                    tracing::error!("删除任务失败: {err}");
                    if let Ok(list) = dispatcher.list().await {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                let mut store = store_clone.lock();
                                store.replace_all(list);
                                refresh_ui(&ui, &store);
                            }
                        });
                    }
                }
            });
        });
    }

    // Pause All
    {
        let dispatcher = dispatcher.clone();
        main_window.on_pause_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Downloading)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.pause(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    // Resume All
    {
        let dispatcher = dispatcher.clone();
        main_window.on_resume_all(move || {
            let dispatcher = dispatcher.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    for item in list {
                        if matches!(item.state, DownloadState::Paused)
                            && let Ok(task_id) = TaskId::from_wire_string(&item.id)
                        {
                            let _ = dispatcher.resume(&task_id).await;
                        }
                    }
                }
            });
        });
    }

    // Clear Completed: drop the records of every finished task (files stay on
    // disk, matching the web client's "clear completed" action).
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_clear_completed(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            let active_inspector_id = active_inspector_id_clone.clone();

            let completed_ids = {
                let mut store = store_clone.lock();
                let ids = store.completed_ids();
                for id in &ids {
                    store.remove(id);
                }
                ids
            };

            let lang = store_clone.lock().language();
            if completed_ids.is_empty() {
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_clear_completed_none(lang).to_string(),
                    "info",
                    Duration::from_secs(4),
                );
                return;
            }

            if let Some(ui) = ui_weak.upgrade() {
                let store = store_clone.lock();
                refresh_ui(&ui, &store);
                if let Some(ref current_id) = *active_inspector_id.lock()
                    && completed_ids.contains(current_id)
                {
                    ui.set_show_inspector(false);
                }
            }

            tokio::spawn(async move {
                let mut cleared = 0usize;
                for id in &completed_ids {
                    if let Ok(task_id) = TaskId::from_wire_string(id)
                        && dispatcher.remove(&task_id).await.is_ok()
                    {
                        cleared += 1;
                    }
                }
                if cleared == 0 {
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_clear_completed_failed(lang).to_string(),
                        "error",
                        Duration::from_secs(5),
                    );
                    return;
                }
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_clear_completed(cleared, lang),
                    "success",
                    Duration::from_secs(4),
                );

                if let Ok(list) = dispatcher.list().await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut store = store_clone.lock();
                            store.replace_all(list);
                            refresh_ui(&ui, &store);
                        }
                    });
                }
            });
        });
    }

    // Pick Torrent File (Native Dialog) + start file pre-selection preview
    {
        let ui_weak = main_window.as_weak();
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();

        main_window.on_copy_task_url(move |url| {
            let url_str = url.to_string();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&url_str);
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_link_copied(store_clone.lock().language()).to_string(),
                        "success",
                        Duration::from_secs(3),
                    );
                }
            });
        });
    }

    // Copy Task Name to Clipboard
    {
        let store_clone = store.clone();
        let toast_queue_clone = toast_queue.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_copy_task_name(move |name| {
            let name_str = name.to_string();
            let store_clone = store_clone.clone();
            let toast_queue = toast_queue_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&name_str);
                    push_toast(
                        &ui_weak,
                        &toast_queue,
                        i18n::format_toast_filename_copied(store_clone.lock().language()).to_string(),
                        "success",
                        Duration::from_secs(3),
                    );
                }
            });
        });
    }

    // Purge Single Task
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        let active_inspector_id_clone = active_inspector_id.clone();
        main_window.on_purge_single_task(move |id_str| {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            let active_inspector_id = active_inspector_id_clone.clone();
            let id = id_str.to_string();

            // 1. Immediate optimistic UI removal
            {
                let mut store = store_clone.lock();
                store.remove(&id);
            }
            if let Some(ui) = ui_weak.upgrade() {
                let store = store_clone.lock();
                refresh_ui(&ui, &store);
                if let Some(ref current_id) = *active_inspector_id.lock()
                    && current_id == &id
                {
                    ui.set_show_inspector(false);
                }
            }

            tokio::spawn(async move {
                if let Ok(task_id) = TaskId::from_wire_string(&id)
                    && let Err(err) = dispatcher.purge(&task_id).await
                {
                    tracing::error!("彻底删除任务失败: {err}");
                    if let Ok(list) = dispatcher.list().await {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak.upgrade() {
                                    let mut store = store_clone.lock();
                                    store.replace_all(list);
                                    refresh_ui(&ui, &store);
                            }
                        });
                    }
                }
            });
        });
    }


    // Background refresh
    {
        let dispatcher = dispatcher.clone();
        let store_clone = store.clone();
        let ui_weak = main_window.as_weak();
        main_window.on_background_refresh(move || {
            let dispatcher = dispatcher.clone();
            let store_clone = store_clone.clone();
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                if let Ok(list) = dispatcher.list().await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let mut store = store_clone.lock();
                            store.replace_all(list);
                            refresh_ui(&ui, &store);
                        }
                    });
                }
            });
        });
    }
}
