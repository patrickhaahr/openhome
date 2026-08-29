import { useEffect, useMemo, useReducer, useRef } from "react";

import { parseConfiguration, type Configuration } from "../domain/configuration";
import type { Result } from "../domain/result";
import type { ConfigurationStore } from "../infrastructure/configuration-store";
import {
  createOpenHomeApi,
  type IrStatus,
  type OpenHomeApi,
} from "../infrastructure/open-home-api";

/** A top-level destination in the configured app. */
export type TopLevelTab = "home" | "television" | "speaker" | "away" | "server" | "docker";

/** The state of IR status loading. */
export type IrState =
  | { readonly tag: "loading" }
  | { readonly tag: "loaded"; readonly status: IrStatus }
  | { readonly tag: "error"; readonly message: string };

/** Per-control progress and failure state. */
export type CommandState = {
  readonly sending: ReadonlySet<string>;
  readonly error: { readonly command: string; readonly message: string } | null;
};

/** The configured application's active state. */
export type ReadyState = {
  readonly tag: "ready";
  readonly configuration: Configuration;
  readonly selectedTab: TopLevelTab;
  readonly ir: IrState;
  readonly edifier: CommandState;
  readonly television: CommandState;
  readonly light: CommandState;
};

/** All valid states presented by the application. */
export type OpenHomeState =
  | { readonly tag: "loading" }
  | {
      readonly tag: "configuration";
      readonly mode: "setup" | "reconfigure";
      readonly previous: ReadyState | null;
      readonly baseUrl: string;
      readonly apiKey: string;
      readonly isSaving: boolean;
      readonly error: string | null;
    }
  | ReadyState;

/** User actions accepted by the application model. */
export type OpenHomeActions = {
  readonly updateBaseUrl: (value: string) => void;
  readonly updateApiKey: (value: string) => void;
  readonly submitConfiguration: () => void;
  readonly openReconfiguration: () => void;
  readonly cancelReconfiguration: () => void;
  readonly selectTab: (tab: TopLevelTab) => void;
  readonly retryIrStatus: () => void;
  readonly sendEdifierCommand: (command: string) => void;
  readonly sendTelevisionCommand: (command: string) => void;
  readonly sendLightCommand: (command: "on" | "off") => void;
};

type Event =
  | {
      readonly type: "configurationLoaded";
      readonly configuration: Configuration | null;
      readonly error: string | null;
    }
  | { readonly type: "baseUrlChanged"; readonly value: string }
  | { readonly type: "apiKeyChanged"; readonly value: string }
  | { readonly type: "configurationSaving" }
  | { readonly type: "configurationFailed"; readonly message: string }
  | { readonly type: "configurationSaved"; readonly configuration: Configuration }
  | { readonly type: "reconfigurationOpened" }
  | { readonly type: "reconfigurationCancelled" }
  | { readonly type: "tabSelected"; readonly tab: TopLevelTab }
  | { readonly type: "irLoading" }
  | { readonly type: "irLoaded"; readonly status: IrStatus }
  | { readonly type: "irFailed"; readonly message: string }
  | { readonly type: "commandStarted"; readonly target: CommandTarget; readonly command: string }
  | {
      readonly type: "commandFinished";
      readonly target: CommandTarget;
      readonly command: string;
      readonly error: string | null;
    };

type CommandTarget = "edifier" | "television" | "light";

const emptyCommandState: CommandState = { sending: new Set(), error: null };

/** Coordinate setup, persistence, and remote commands for the OpenHome UI. */
export function useOpenHome(
  store: ConfigurationStore,
): readonly [OpenHomeState, OpenHomeActions, OpenHomeApi | null] {
  const [state, dispatch] = useReducer(reduce, { tag: "loading" });
  const stateRef = useRef(state);
  const generation = useRef(0);
  const refreshGeneration = useRef(0);
  stateRef.current = state;
  const configuration = state.tag === "ready" ? state.configuration : null;
  const api = useMemo(
    () => (configuration === null ? null : createOpenHomeApi(configuration)),
    [configuration],
  );

  useEffect(() => {
    let active = true;
    void store.load().then((result) => {
      if (!active) {
        return;
      }
      if (!result.ok) {
        dispatch({ type: "configurationLoaded", configuration: null, error: result.error });
        return;
      }
      dispatch({ type: "configurationLoaded", configuration: result.value, error: null });
      if (result.value !== null) {
        void refreshIr(createOpenHomeApi(result.value));
      }
    });
    return () => {
      active = false;
    };
  }, [store]);

  async function refreshIr(api: OpenHomeApi): Promise<void> {
    const refresh = refreshGeneration.current + 1;
    refreshGeneration.current = refresh;
    dispatch({ type: "irLoading" });
    const result = await api.getIrStatus();
    if (refreshGeneration.current !== refresh) {
      return;
    }
    dispatch(
      result.ok
        ? { type: "irLoaded", status: result.value }
        : { type: "irFailed", message: result.error },
    );
  }

  async function saveConfiguration(): Promise<void> {
    const current = stateRef.current;
    if (current.tag !== "configuration" || current.isSaving) {
      return;
    }

    const parsed = parseConfiguration(current.baseUrl, current.apiKey);
    if (!parsed.ok) {
      dispatch({ type: "configurationFailed", message: parsed.error });
      return;
    }

    dispatch({ type: "configurationSaving" });
    const api = createOpenHomeApi(parsed.value);
    const validation = await api.validateConfiguration();
    if (!validation.ok) {
      dispatch({ type: "configurationFailed", message: validation.error });
      return;
    }

    const saved = await store.save(parsed.value);
    if (!saved.ok) {
      dispatch({ type: "configurationFailed", message: saved.error });
      return;
    }

    generation.current += 1;
    dispatch({ type: "configurationSaved", configuration: parsed.value });
    await refreshIr(api);
  }

  function retryIr(): void {
    const current = stateRef.current;
    if (current.tag !== "ready" || current.ir.tag === "loading") {
      return;
    }
    void refreshIr(createOpenHomeApi(current.configuration));
  }

  function sendIr(
    target: "edifier" | "television",
    remote: "edifier" | "lgtv",
    command: string,
  ): void {
    const current = stateRef.current;
    if (current.tag !== "ready" || current.ir.tag !== "loaded") {
      return;
    }
    const available =
      remote === "edifier" ? current.ir.status.edifierCommands : current.ir.status.lgTvCommands;
    if (!available.has(command) || current[target].sending.has(command)) {
      return;
    }
    void runCommand(
      target,
      command,
      createOpenHomeApi(current.configuration).sendIrCommand(remote, command),
    );
  }

  function sendLight(command: "on" | "off"): void {
    const current = stateRef.current;
    if (current.tag !== "ready" || current.light.sending.size > 0) {
      return;
    }
    void runCommand(
      "light",
      command,
      createOpenHomeApi(current.configuration).sendLightCommand(command),
    );
  }

  async function runCommand(
    target: CommandTarget,
    command: string,
    operation: Promise<Result<void>>,
  ): Promise<void> {
    const operationGeneration = generation.current;
    dispatch({ type: "commandStarted", target, command });
    const result = await operation;
    if (generation.current !== operationGeneration) {
      return;
    }
    dispatch({ type: "commandFinished", target, command, error: result.ok ? null : result.error });
  }

  const actions: OpenHomeActions = {
    updateBaseUrl(value) {
      dispatch({ type: "baseUrlChanged", value });
    },
    updateApiKey(value) {
      dispatch({ type: "apiKeyChanged", value });
    },
    submitConfiguration() {
      void saveConfiguration();
    },
    openReconfiguration() {
      generation.current += 1;
      refreshGeneration.current += 1;
      dispatch({ type: "reconfigurationOpened" });
    },
    cancelReconfiguration() {
      dispatch({ type: "reconfigurationCancelled" });
    },
    selectTab(tab) {
      dispatch({ type: "tabSelected", tab });
      if (
        (tab === "television" || tab === "speaker") &&
        stateRef.current.tag === "ready" &&
        stateRef.current.ir.tag === "error"
      ) {
        retryIr();
      }
    },
    retryIrStatus: retryIr,
    sendEdifierCommand(command) {
      sendIr("edifier", "edifier", command);
    },
    sendTelevisionCommand(command) {
      sendIr("television", "lgtv", command);
    },
    sendLightCommand: sendLight,
  };

  return [state, actions, api] as const;
}

/** Apply a synchronous event to application state. */
export function reduce(state: OpenHomeState, event: Event): OpenHomeState {
  switch (event.type) {
    case "configurationLoaded":
      return event.configuration === null
        ? configurationState("setup", null, "", "", event.error)
        : readyState(event.configuration);
    case "baseUrlChanged":
      return state.tag === "configuration"
        ? { ...state, baseUrl: event.value, error: null }
        : state;
    case "apiKeyChanged":
      return state.tag === "configuration" ? { ...state, apiKey: event.value, error: null } : state;
    case "configurationSaving":
      return state.tag === "configuration" ? { ...state, isSaving: true, error: null } : state;
    case "configurationFailed":
      return state.tag === "configuration"
        ? { ...state, isSaving: false, error: event.message }
        : state;
    case "configurationSaved":
      return readyState(
        event.configuration,
        state.tag === "configuration" && state.previous !== null
          ? state.previous.selectedTab
          : "home",
      );
    case "reconfigurationOpened":
      return state.tag === "ready"
        ? configurationState(
            "reconfigure",
            withoutPendingCommands(state),
            state.configuration.baseUrl,
            state.configuration.apiKey,
            null,
          )
        : state;
    case "reconfigurationCancelled":
      return state.tag === "configuration" && state.previous !== null ? state.previous : state;
    case "tabSelected":
      return state.tag === "ready" ? { ...state, selectedTab: event.tab } : state;
    case "irLoading":
      return state.tag === "ready" ? { ...state, ir: { tag: "loading" } } : state;
    case "irLoaded":
      return state.tag === "ready"
        ? { ...state, ir: { tag: "loaded", status: event.status } }
        : state;
    case "irFailed":
      return state.tag === "ready"
        ? { ...state, ir: { tag: "error", message: event.message } }
        : state;
    case "commandStarted":
      return updateCommands(state, event.target, (current) => ({
        sending: new Set([...current.sending, event.command]),
        error: null,
      }));
    case "commandFinished":
      return updateCommands(state, event.target, (current) => ({
        sending: new Set([...current.sending].filter((command) => command !== event.command)),
        error:
          event.error === null ? current.error : { command: event.command, message: event.error },
      }));
  }
}

function configurationState(
  mode: "setup" | "reconfigure",
  previous: ReadyState | null,
  baseUrl: string,
  apiKey: string,
  error: string | null,
): OpenHomeState {
  return { tag: "configuration", mode, previous, baseUrl, apiKey, isSaving: false, error };
}

function readyState(configuration: Configuration, selectedTab: TopLevelTab = "home"): ReadyState {
  return {
    tag: "ready",
    configuration,
    selectedTab,
    ir: { tag: "loading" },
    edifier: emptyCommandState,
    television: emptyCommandState,
    light: emptyCommandState,
  };
}

function withoutPendingCommands(state: ReadyState): ReadyState {
  return {
    ...state,
    edifier: { ...state.edifier, sending: new Set() },
    television: { ...state.television, sending: new Set() },
    light: { ...state.light, sending: new Set() },
  };
}

function updateCommands(
  state: OpenHomeState,
  target: CommandTarget,
  update: (current: CommandState) => CommandState,
): OpenHomeState {
  return state.tag === "ready" ? { ...state, [target]: update(state[target]) } : state;
}
