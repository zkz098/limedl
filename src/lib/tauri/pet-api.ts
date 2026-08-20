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
