import { invoke } from "#invoke";

export function petShow() {
  return invoke<void>("pet_show");
}

export function petHide() {
  return invoke<void>("pet_hide");
}

export function petClose() {
  return invoke<void>("pet_close");
}

export function petSetScale(scale: number) {
  return invoke<void>("pet_set_scale", { scale });
}

export function petSetIgnoreCursorEvents(ignore: boolean) {
  return invoke<void>("pet_set_ignore_cursor_events", { ignore });
}

export function petStartDrag() {
  return invoke<void>("pet_start_drag");
}

export function petUpdatePosition(x: number, y: number) {
  return invoke<void>("pet_update_position", { x, y });
}

export function petSetEnabled(enabled: boolean) {
  return invoke<void>("pet_set_enabled", { enabled });
}

export interface PetMenuState {
  hasActive: boolean;
  speedLimitActive: boolean;
  gameMode: boolean;
  mainVisible: boolean;
}

export function petGetMenuState() {
  return invoke<PetMenuState>("pet_get_menu_state");
}

export function petTogglePauseAll() {
  return invoke<void>("pet_toggle_pause_all");
}

export function petToggleSpeedLimit() {
  return invoke<void>("pet_toggle_speed_limit");
}

export function petToggleGameMode() {
  return invoke<void>("pet_toggle_game_mode");
}

export function petOpenDownloadDir() {
  return invoke<void>("pet_open_download_dir");
}

export function petShowMain() {
  return invoke<void>("pet_show_main");
}

export function petOpenSettings() {
  return invoke<void>("pet_open_settings");
}

export function petQuit() {
  return invoke<void>("pet_quit");
}
