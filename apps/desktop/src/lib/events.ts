import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppEvent } from "./contracts";

const APP_EVENT_CHANNEL = "app-event";

/** Subscribe to all `app-event` payloads from the M00 Tauri bridge. */
export function listenAppEvents<T = unknown>(
  handler: (event: AppEvent<T>) => void,
): Promise<UnlistenFn> {
  return listen<AppEvent<T>>(APP_EVENT_CHANNEL, (ev) => {
    handler(ev.payload);
  });
}

/** Filter events by `type` (single or multiple). */
export function matchesEventType(
  event: AppEvent,
  types: string | readonly string[],
): boolean {
  const list = typeof types === "string" ? [types] : types;
  return list.length === 0 || list.includes(event.type);
}
