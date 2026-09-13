import { describe, expect, it } from "vitest";

import type { BodyWeightEntry, BodyWeightInput, Profile, ProfileInput } from "../domain/fitness";
import { failure, success, type Result } from "../domain/result";
import {
  createBodyController,
  initialState,
  reduce,
  type BodyEvent,
  type BodyState,
} from "./use-body";

const ENTRY: BodyWeightEntry = { id: 3, date: "2026-09-12", weightKg: 72.5 };
const PROFILE: Profile = { heightCm: 182.5, sex: "male" };
const WEIGHT_INPUT: BodyWeightInput = { date: "2026-09-12", weightKg: 72.5 };
const PROFILE_INPUT: ProfileInput = { heightCm: 182.5, sex: "male" };

/** A scripted fake of the body weight and profile adapter surface with manually resolved responses. */
function fakeApi() {
  const calls: Array<
    | { readonly kind: "listBodyWeight" }
    | { readonly kind: "createBodyWeight"; readonly input: BodyWeightInput }
    | { readonly kind: "getProfile" }
    | { readonly kind: "updateProfile"; readonly input: ProfileInput }
  > = [];
  const listPending: Array<(result: Result<readonly BodyWeightEntry[]>) => void> = [];
  const savePending: Array<(result: Result<BodyWeightEntry>) => void> = [];
  const profilePending: Array<(result: Result<Profile>) => void> = [];
  const api = {
    listBodyWeight: () => {
      calls.push({ kind: "listBodyWeight" });
      return new Promise<Result<readonly BodyWeightEntry[]>>((resolve) => {
        listPending.push(resolve);
      });
    },
    createBodyWeight: (input: BodyWeightInput) => {
      calls.push({ kind: "createBodyWeight", input });
      return new Promise<Result<BodyWeightEntry>>((resolve) => {
        savePending.push(resolve);
      });
    },
    getProfile: () => {
      calls.push({ kind: "getProfile" });
      return new Promise<Result<Profile>>((resolve) => {
        profilePending.push(resolve);
      });
    },
    updateProfile: (input: ProfileInput) => {
      calls.push({ kind: "updateProfile", input });
      return new Promise<Result<Profile>>((resolve) => {
        profilePending.push(resolve);
      });
    },
  };
  return { api, calls, listPending, savePending, profilePending };
}

function harness() {
  const fake = fakeApi();
  const events: BodyEvent[] = [];
  const controller = createBodyController({
    api: fake.api,
    emit: (event) => events.push(event),
  });
  return { ...fake, controller, events };
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function replay(events: ReadonlyArray<BodyEvent>): BodyState {
  return events.reduce((state, event) => reduce(state, event), initialState);
}

describe("body state machine", () => {
  it("loads entries and the profile into the ready state", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([ENTRY]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    const state = replay(h.events);
    expect(state.entries).toEqual([ENTRY]);
    expect(state.profile).toEqual({ tag: "loaded", profile: PROFILE });
    expect(state.refreshing).toBe(false);
    expect(state.error).toBeNull();
  });

  it("surfaces a failed load without losing entries", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([ENTRY]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    h.controller.refresh();
    h.listPending[1]?.(failure("Couldn't reach the Axum API."));
    h.profilePending[1]?.(success(PROFILE));
    await settle();

    const state = replay(h.events);
    expect(state.error).toBe("Couldn't reach the Axum API.");
    expect(state.entries).toEqual([ENTRY]);
    expect(state.refreshing).toBe(false);
  });

  it("surfaces a failed profile load", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([]));
    h.profilePending[0]?.(failure("Couldn't load the profile."));
    await settle();

    expect(replay(h.events).profile).toEqual({
      tag: "error",
      message: "Couldn't load the profile.",
    });
  });

  it("records a body weight entry and reloads the list", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([ENTRY]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    h.controller.saveWeight(WEIGHT_INPUT);
    h.savePending[0]?.(success({ id: 4, date: "2026-09-13", weightKg: 73 }));
    await settle();
    h.listPending[1]?.(success([ENTRY, { id: 4, date: "2026-09-13", weightKg: 73 }]));
    h.profilePending[1]?.(success(PROFILE));
    await settle();

    const state = replay(h.events);
    expect(state.saving).toBe(false);
    expect(state.entries.map((entry) => entry.id)).toEqual([3, 4]);
    expect(h.calls.filter((call) => call.kind === "createBodyWeight")).toEqual([
      { kind: "createBodyWeight", input: WEIGHT_INPUT },
    ]);
  });

  it("keeps entries and surfaces the error on a failed save", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([ENTRY]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    h.controller.saveWeight(WEIGHT_INPUT);
    h.savePending[0]?.(failure("Body weight for that date is already recorded."));
    await settle();

    const state = replay(h.events);
    expect(state.saving).toBe(false);
    expect(state.saveError).toBe("Body weight for that date is already recorded.");
    expect(state.entries).toEqual([ENTRY]);
  });

  it("lets an in-flight refresh resolve when a save fails, clearing the spinner", async () => {
    const h = harness();
    h.controller.refresh();
    h.controller.saveWeight(WEIGHT_INPUT);
    h.savePending[0]?.(failure("Body weight for that date is already recorded."));
    h.listPending[0]?.(success([ENTRY]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    const state = replay(h.events);
    expect(state.saveError).toBe("Body weight for that date is already recorded.");
    expect(state.error).toBeNull();
    expect(state.refreshing).toBe(false);
    expect(state.entries).toEqual([ENTRY]);
    expect(state.profile).toEqual({ tag: "loaded", profile: PROFILE });
  });

  it("saves the profile from the returned row without a reload", async () => {
    const h = harness();
    h.controller.refresh();
    h.listPending[0]?.(success([]));
    h.profilePending[0]?.(success(PROFILE));
    await settle();

    h.controller.saveProfile(PROFILE_INPUT);
    h.profilePending[1]?.(
      success({ heightCm: 182.5, sex: "male" }),
    );
    await settle();

    const state = replay(h.events);
    expect(state.saving).toBe(false);
    expect(state.profile).toEqual({ tag: "loaded", profile: { heightCm: 182.5, sex: "male" } });
    expect(h.calls.filter((call) => call.kind === "updateProfile")).toEqual([
      { kind: "updateProfile", input: PROFILE_INPUT },
    ]);
  });

  it("ignores a mutation while another is in flight", async () => {
    const h = harness();
    h.controller.saveWeight(WEIGHT_INPUT);
    h.controller.saveWeight(WEIGHT_INPUT);
    h.savePending[0]?.(success(ENTRY));
    await settle();

    expect(h.calls.filter((call) => call.kind === "createBodyWeight")).toHaveLength(1);
  });
});
