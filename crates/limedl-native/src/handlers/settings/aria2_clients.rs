//! Per-client Aria2 RPC token rows.
//!
//! Rows live in the UI model until Save, exactly like the media overrides. The
//! callbacks here only manage that model: token generation + Argon2 hashing on
//! add/regenerate, in-place name edits, removal and a clipboard copy of a
//! freshly generated token. Persisting the rows is `handlers::settings::dialog`'s
//! job (`parse_aria2_clients`).

use std::time::Duration;

use slint::{Model, VecModel};

use crate::Aria2ClientItem;
use crate::bridge::{Aria2ClientText, aria2_clients_to_slint};
use crate::context::AppContext;
use crate::handlers::common::{read_ui, with_ui};
use crate::i18n::{self};
use crate::toast::push_toast;
use crate::ui_sync::{push_aria2_client_rows, read_aria2_client_rows};
use crate::MainWindow;

pub fn register(ctx: &AppContext) {
    let ui = &ctx.ui;
    let ui_weak = ctx.ui_weak.clone();
    let store = ctx.store.clone();
    let toast_queue = ctx.toast_queue.clone();

    // Add: mint a token, hash it, and reveal the plaintext in the new row.
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_aria2_client_add(move || {
            let lang = store.lock().language();
            let mut failure = None;
            with_ui(&ui_weak, |ui| {
                let mut rows = read_aria2_client_rows(&ui);
                let name = i18n::format_aria2_client_default_name(rows.len() + 1, lang);
                match Aria2ClientText::generate(name) {
                    Ok(row) => rows.push(row),
                    Err(err) => failure = Some(err),
                }
                push_aria2_client_rows(&ui, &rows);
            });
            if let Some(err) = failure {
                tracing::error!("生成 Aria2 客户端令牌失败: {err}");
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_aria2_token_failed(&err, lang),
                    "error",
                    Duration::from_secs(6),
                );
            }
        });
    }

    {
        let ui_weak = ui_weak.clone();
        ui.on_aria2_client_remove(move |index| {
            with_ui(&ui_weak, |ui| {
                let mut rows = read_aria2_client_rows(&ui);
                let index = index.max(0) as usize;
                if index < rows.len() {
                    rows.remove(index);
                }
                push_aria2_client_rows(&ui, &rows);
            });
        });
    }

    // Name edits update the row in place so the focused input keeps its caret.
    {
        let ui_weak = ui_weak.clone();
        ui.on_aria2_client_name_edited(move |index, name| {
            edit_client_row(ui_weak.clone(), index, |row| row.name = name.to_string());
        });
    }

    // Regenerate: replace the token and reveal the new plaintext.
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_aria2_client_regenerate(move |index| {
            let lang = store.lock().language();
            let mut failure = None;
            with_ui(&ui_weak, |ui| {
                let mut rows = read_aria2_client_rows(&ui);
                let index = index.max(0) as usize;
                if let Some(row) = rows.get_mut(index)
                    && let Err(err) = row.regenerate()
                {
                    failure = Some(err);
                }
                push_aria2_client_rows(&ui, &rows);
            });
            if let Some(err) = failure {
                tracing::error!("重新生成 Aria2 客户端令牌失败: {err}");
                push_toast(
                    &ui_weak,
                    &toast_queue,
                    i18n::format_toast_aria2_token_failed(&err, lang),
                    "error",
                    Duration::from_secs(6),
                );
            }
        });
    }

    // Copy the revealed plaintext before the dialog is saved/closed (after that
    // it is gone — only the hash is kept).
    {
        let ui_weak = ui_weak.clone();
        let store = store.clone();
        let toast_queue = toast_queue.clone();
        ui.on_aria2_client_copy_token(move |index| {
            let token = read_ui(&ui_weak, |ui| {
                ui.get_aria2_clients()
                    .iter()
                    .nth(index.max(0) as usize)
                    .map(|item| item.token.to_string())
            })
            .flatten()
            .unwrap_or_default();
            if token.is_empty() {
                return;
            }
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => {
                    if clipboard.set_text(token).is_ok() {
                        let lang = store.lock().language();
                        push_toast(
                            &ui_weak,
                            &toast_queue,
                            i18n::format_toast_aria2_token_copied(lang).to_string(),
                            "success",
                            Duration::from_secs(3),
                        );
                    }
                }
                Err(err) => tracing::warn!("剪贴板不可用: {err}"),
            }
        });
    }
}

/// Mutate one row in place and write it back through `set_row_data`.
///
/// Rebuilding the whole model would drop the caret of an in-progress name edit;
/// updating a single row keeps the text the user is typing.
fn edit_client_row(
    ui_weak: slint::Weak<MainWindow>,
    index: i32,
    edit: impl FnOnce(&mut Aria2ClientText),
) {
    with_ui(&ui_weak, |ui| {
        let model = ui.get_aria2_clients();
        let Some(vec_model) = model.as_any().downcast_ref::<VecModel<Aria2ClientItem>>() else {
            return;
        };
        let index = index.max(0) as usize;
        // Out-of-range indices are ignored: the model and the store can briefly
        // disagree while the list is rebuilt.
        let Some(item) = vec_model.row_data(index) else {
            return;
        };
        let mut row = Aria2ClientText {
            id: item.id.to_string(),
            name: item.name.to_string(),
            token: item.token.to_string(),
            token_hash: item.token_hash.to_string(),
            created_at_ms: item.created_at_ms.parse::<i64>().unwrap_or(0),
        };
        edit(&mut row);
        let Some(refreshed) = aria2_clients_to_slint(std::slice::from_ref(&row))
            .into_iter()
            .next()
        else {
            return;
        };
        vec_model.set_row_data(index, refreshed);
    });
}
