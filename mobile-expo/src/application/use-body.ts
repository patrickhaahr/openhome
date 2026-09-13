import { useEffect, useMemo, useReducer, useRef } from "react";

import type { BodyWeightEntry, BodyWeightInput, Profile, ProfileInput } from "../domain/fitness";
import type { Result } from "../domain/result";
import type { FitnessApi } from "../infrastructure/open-home-api";

/** The body weight log view and its profile form. */
export type BodyState = {
  readonly entries: readonly BodyWeightEntry[];
  /** A history refresh is in flight; the previous list stays visible. */
  readonly refreshing: boolean;
  /** A save is in flight. */
  readonly saving: boolean;
  readonly profile:
    | { readonly tag: "loading" }
    | { readonly tag: "loaded"; readonly profile: Profile }
    | { readonly tag: "error"; readonly message: string };
  /** The last history load failure; independent of save failures. */
  readonly error: string | null;
  /** The last save failure; list loads arriving later must not clear it. */
  readonly saveError: string | null;
};

/** User actions accepted by the body weight log view. */
export type BodyActions = {
  readonly refresh: () => void;
  /** The input is validated on the client by the caller before this is invoked. */
  readonly saveWeight: (input: BodyWeightInput) => void;
  /** The input is validated on the client by the caller before this is invoked. */
  readonly saveProfile: (input: ProfileInput) => void;
};

export type BodyEvent =
  | { readonly type: "listStarted" }
  | { readonly type: "listSucceeded"; readonly entries: readonly BodyWeightEntry[] }
  | { readonly type: "listFailed"; readonly message: string }
  | { readonly type: "profileStarted" }
  | { readonly type: "profileSucceeded"; readonly profile: Profile }
  | { readonly type: "profileFailed"; readonly message: string }
  | { readonly type: "saveStarted" }
  | { readonly type: "saveSucceeded" }
  | { readonly type: "saveFailed"; readonly message: string }
  /** A cancelled controller's in-flight mutation resolved; its flag must clear. */
  | { readonly type: "superseded" };

/** Drives the body weight log and profile against the Axum API adapter, emitting events. */
export function createBodyController(deps: {
  readonly api: Pick<FitnessApi, "listBodyWeight" | "createBodyWeight" | "getProfile" | "updateProfile">;
  readonly emit: (event: BodyEvent) => void;
}) {
  let listToken = 0;
  let profileToken = 0;
  let mutateToken = 0;
  let busy = false;

  async function load(): Promise<void> {
    const listCurrent = ++listToken;
    const profileCurrent = ++profileToken;
    deps.emit({ type: "listStarted" });
    deps.emit({ type: "profileStarted" });
    const [listResult, profileResult] = await Promise.all([
      deps.api.listBodyWeight(),
      deps.api.getProfile(),
    ]);
    if (listToken === listCurrent) {
      deps.emit(
        listResult.ok
          ? { type: "listSucceeded", entries: listResult.value }
          : { type: "listFailed", message: listResult.error },
      );
    }
    if (profileToken === profileCurrent) {
      deps.emit(
        profileResult.ok
          ? { type: "profileSucceeded", profile: profileResult.value }
          : { type: "profileFailed", message: profileResult.error },
      );
    }
  }

  /** Run one mutation, reloading history and profile afterwards when it succeeded. */
  async function mutate(run: () => Promise<Result<unknown>>): Promise<void> {
    if (busy) {
      return;
    }
    busy = true;
    const current = ++mutateToken;
    deps.emit({ type: "saveStarted" });
    const result = await run();
    if (mutateToken !== current) {
      busy = false;
      deps.emit({ type: "superseded" });
      return;
    }
    busy = false;
    if (!result.ok) {
      // A failed mutation changed nothing, so any refresh still in flight is
      // not stale; let it resolve instead of dropping it and sticking the
      // refreshing flag on.
      deps.emit({ type: "saveFailed", message: result.error });
      return;
    }
    // A refresh in flight since before this successful mutation carries a
    // stale snapshot; drop it so the post-mutation reload is authoritative.
    listToken += 1;
    profileToken += 1;
    deps.emit({ type: "saveSucceeded" });
    void load();
  }

  return {
    refresh(): void {
      void load();
    },
    saveWeight(input: BodyWeightInput): void {
      void mutate(() => deps.api.createBodyWeight(input));
    },
    saveProfile(input: ProfileInput): void {
      void mutate(() => deps.api.updateProfile(input));
    },
    cancel(): void {
      listToken += 1;
      profileToken += 1;
      mutateToken += 1;
      if (busy) {
        busy = false;
        deps.emit({ type: "superseded" });
      }
    },
  };
}

/** Coordinate body weight log and profile state for the Fitness Tab UI. */
export function useBody(api: FitnessApi | null): readonly [BodyState, BodyActions] {
  const [state, dispatch] = useReducer(reduce, initialState);
  const controller = useRef<ReturnType<typeof createBodyController> | null>(null);

  useEffect(() => {
    if (api === null) {
      controller.current = null;
      return;
    }
    const current = createBodyController({ api, emit: dispatch });
    controller.current = current;
    current.refresh();
    return () => current.cancel();
  }, [api]);

  return [
    state,
    useMemo<BodyActions>(
      () => ({
        refresh: () => controller.current?.refresh(),
        saveWeight: (input) => controller.current?.saveWeight(input),
        saveProfile: (input) => controller.current?.saveProfile(input),
      }),
      [],
    ),
  ];
}

export const initialState: BodyState = {
  entries: [],
  refreshing: false,
  saving: false,
  profile: { tag: "loading" },
  error: null,
  saveError: null,
};

/** Apply a machine event to body weight log state. */
export function reduce(state: BodyState, event: BodyEvent): BodyState {
  switch (event.type) {
    case "listStarted":
      return { ...state, refreshing: true };
    case "listSucceeded":
      return { ...state, entries: event.entries, refreshing: false, error: null };
    case "listFailed":
      return { ...state, refreshing: false, error: event.message };
    case "profileStarted":
      return {
        ...state,
        profile: state.profile.tag === "loaded" ? state.profile : { tag: "loading" },
      };
    case "profileSucceeded":
      return { ...state, profile: { tag: "loaded", profile: event.profile } };
    case "profileFailed":
      return {
        ...state,
        profile:
          state.profile.tag === "loaded"
            ? state.profile
            : { tag: "error", message: event.message },
      };
    case "saveStarted":
      return { ...state, saving: true, saveError: null };
    case "saveSucceeded":
      return { ...state, saving: false, saveError: null };
    case "saveFailed":
      return { ...state, saving: false, saveError: event.message };
    case "superseded":
      return { ...state, saving: false };
  }
}
