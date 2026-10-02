//! Method dispatch table for JSON-RPC requests.

use super::{ERR_METHOD_NOT_FOUND, JsonRpcError, RpcContext, Value, handle_add_torrent, handle_add_uri, handle_change_global_option, handle_get_files, handle_get_global_option, handle_get_option, handle_get_peers, handle_get_session_info, handle_get_uris, handle_global_stat, handle_list_methods, handle_list_notifications, handle_multicall, handle_pause, handle_pause_all, handle_purge_download_result, handle_remove, handle_save_session, handle_shutdown, handle_tell_active, handle_tell_status, handle_tell_stopped, handle_tell_waiting, handle_unpause, handle_unpause_all, handle_version, make_error};

pub(crate) async fn dispatch_method(
    ctx: &RpcContext,
    method: &str,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    match method {
        "aria2.addUri" => handle_add_uri(ctx, params).await,
        "aria2.addTorrent" => handle_add_torrent(ctx, params).await,
        "aria2.multicall" | "system.multicall" => handle_multicall(ctx, params).await,
        "aria2.pause" | "aria2.forcePause" => handle_pause(ctx, params).await,
        "aria2.unpause" => handle_unpause(ctx, params).await,
        "aria2.pauseAll" | "aria2.forcePauseAll" => handle_pause_all(ctx).await,
        "aria2.purgeDownloadResult" => handle_purge_download_result(ctx).await,
        "aria2.unpauseAll" => handle_unpause_all(ctx).await,
        "aria2.remove" | "aria2.forceRemove" => handle_remove(ctx, params).await,
        "aria2.tellStatus" => handle_tell_status(ctx, params).await,
        "aria2.tellActive" => handle_tell_active(ctx, params).await,
        "aria2.tellWaiting" => handle_tell_waiting(ctx, params).await,
        "aria2.tellStopped" => handle_tell_stopped(ctx, params).await,
        "aria2.getGlobalStat" => handle_global_stat(ctx).await,
        "aria2.getGlobalOption" => handle_get_global_option(ctx).await,
        "aria2.changeGlobalOption" => handle_change_global_option(ctx, params).await,
        "aria2.getVersion" => Ok(handle_version()),
        "aria2.getFiles" => handle_get_files(ctx, params).await,
        "aria2.getOption" => handle_get_option(ctx, params).await,
        "aria2.getUris" => handle_get_uris(ctx, params).await,
        "aria2.getPeers" => handle_get_peers(ctx, params).await,
        "aria2.getSessionInfo" => Ok(handle_get_session_info(ctx)),
        "aria2.saveSession" => Ok(handle_save_session()),
        "aria2.shutdown" => handle_shutdown(ctx).await,
        "system.listMethods" => Ok(handle_list_methods()),
        "system.listNotifications" => Ok(handle_list_notifications()),
        _ => Err(make_error(
            ERR_METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    }
}
