import { useEffect, useRef } from "react";
import type { AppEvent } from "../lib/contracts";
import { listenAppEvents, matchesEventType } from "../lib/events";

/**
 * React hook — listen to M00 `app-event` bridge, optionally filtered by event type(s).
 */
export function useAppEvent<T = unknown>(
  eventTypes: string | readonly string[] | null,
  handler: (event: AppEvent<T>) => void,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    listenAppEvents<T>((event) => {
      if (eventTypes === null || matchesEventType(event, eventTypes)) {
        handlerRef.current(event);
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [eventTypes]);
}
