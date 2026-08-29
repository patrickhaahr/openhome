import { useEffect, useMemo, useReducer, useRef } from "react";

import type { AdguardStatus } from "../domain/adguard";
import type { Result } from "../domain/result";
import type { AdguardApi } from "../infrastructure/open-home-api";

/** The user-visible state of the AdGuard Protection card. */
export type AdguardState =
  | { readonly tag: "loading" }
  | { readonly tag: "error"; readonly message: string }
  | {
      readonly tag: "ready";
      readonly status: AdguardStatus;
      /** A status refresh is in flight; the previous status stays visible. */
      readonly refreshing: boolean;
      /** An enable/disable/pause action is in flight. */
      readonly acting: boolean;
      readonly error: string | null;
    };

/** User actions accepted by the AdGuard Protection card. */
export type AdguardActions = {
  readonly refresh: () => void;
  readonly enable: () => void;
  readonly disable: () => void;
  readonly pause: (minutes: number) => void;
};

export type AdguardEvent =
  | { readonly type: "loadStarted" }
  | { readonly type: "loadSucceeded"; readonly status: AdguardStatus }
  | { readonly type: "loadFailed"; readonly message: string }
  | { readonly type: "actionStarted" }
  | {
      readonly type: "actionFinished";
      readonly status: AdguardStatus | null;
      readonly error: string | null;
    }
  /** A superseded operation resolved; its result is dropped but its own flag must clear. */
  | { readonly type: "superseded"; readonly of: "refresh" | "action" };

/** Drives the AdGuard machine against the Axum API adapter, emitting events. */
export function createAdguardController(deps: {
  readonly api: AdguardApi;
  readonly emit: (event: AdguardEvent) => void;
}) {
  let token = 0;
  let acting = false;

  async function fetchStatus(): Promise<void> {
    const current = ++token;
    deps.emit({ type: "loadStarted" });
    const result = await deps.api.getStatus();
    if (token !== current) {
      deps.emit({ type: "superseded", of: "refresh" });
      return;
    }
    deps.emit(
      result.ok
        ? { type: "loadSucceeded", status: result.value }
        : { type: "loadFailed", message: result.error },
    );
  }

  async function act(
    operation: (api: AdguardApi) => Promise<Result<AdguardStatus>>,
  ): Promise<void> {
    if (acting) {
      return;
    }
    acting = true;
    const current = ++token;
    deps.emit({ type: "actionStarted" });
    const result = await operation(deps.api);
    acting = false;
    if (token !== current) {
      deps.emit({ type: "superseded", of: "action" });
      return;
    }
    deps.emit({
      type: "actionFinished",
      status: result.ok ? result.value : null,
      error: result.ok ? null : result.error,
    });
  }

  return {
    refresh(): void {
      void fetchStatus();
    },
    enable(): void {
      void act((api) => api.enableProtection());
    },
    disable(): void {
      void act((api) => api.disableProtection());
    },
    pause(minutes: number): void {
      void act((api) => api.pauseProtection(minutes));
    },
    cancel(): void {
      token += 1;
      acting = false;
    },
  };
}

/** Coordinate AdGuard Protection state for the Server Tab UI. */
export function useAdGuard(api: AdguardApi | null): readonly [AdguardState, AdguardActions] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const controller = useRef<ReturnType<typeof createAdguardController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createAdguardController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<AdguardActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        enable: () => controller.current?.enable(),
        disable: () => controller.current?.disable(),
        pause: (minutes) => controller.current?.pause(minutes),
      }),
      [],
    ),
  ];
}

/** Apply a machine event to AdGuard card state. */
export function reduce(state: AdguardState, event: AdguardEvent): AdguardState {
  switch (event.type) {
    case "loadStarted":
      return state.tag === "ready" ? { ...state, refreshing: true } : { tag: "loading" };
    case "loadSucceeded":
      return state.tag === "ready"
        ? { ...state, status: event.status, refreshing: false }
        : {
            tag: "ready",
            status: event.status,
            refreshing: false,
            acting: false,
            error: null,
          };
    case "loadFailed":
      return state.tag === "ready"
        ? { ...state, refreshing: false, error: event.message }
        : { tag: "error", message: event.message };
    case "actionStarted":
      return state.tag === "ready" ? { ...state, acting: true, error: null } : state;
    case "actionFinished":
      return state.tag === "ready"
        ? {
            ...state,
            acting: false,
            status: event.status ?? state.status,
            error: event.error ?? state.error,
          }
        : state;
    case "superseded":
      return state.tag === "ready"
        ? event.of === "refresh"
          ? { ...state, refreshing: false }
          : { ...state, acting: false }
        : state;
  }
}
