import { ActivityIndicator, Pressable, Text, TextInput, View } from "react-native";
import { useEffect, useRef, useState } from "react";

import type { BodyActions, BodyState } from "../../application/use-body";
import { parseBodyWeightInput, parseProfileInput } from "../../domain/fitness";
import { ActionButton, styles as shared } from "../shared";
import { colors } from "../theme";
import { formatDate, styles, today } from "./shared";

type BodyWeightFormState = { readonly date: string; readonly weightKg: string };

/** Render the body weight log: record today's weight, past entries, and the profile. */
export function BodyView({
  state,
  actions,
}: {
  readonly state: BodyState;
  readonly actions: BodyActions;
}) {
  const [form, setForm] = useState<BodyWeightFormState>({ date: today(), weightKg: "" });
  // Untouched profile fields stay undefined so the PATCH omits them and the
  // API keeps the current values; a cleared field becomes "" and sends null.
  const [profileForm, setProfileForm] = useState<{ heightCm?: string; sex?: string }>({});
  const [profileOpen, setProfileOpen] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [profileError, setProfileError] = useState<string | null>(null);
  const wasSaving = useRef(false);

  // Clear the weight form once its submission has succeeded; a failed save
  // keeps the fields so only the failing part has to change.
  useEffect(() => {
    if (wasSaving.current && !state.saving && state.saveError === null) {
      const latest = state.entries[0]?.weightKg;
      setForm({ date: today(), weightKg: latest === undefined ? "" : String(latest) });
      setFormError(null);
    }
    wasSaving.current = state.saving;
  }, [state]);

  function submitWeight(): void {
    const parsed = parseBodyWeightInput(form.date, form.weightKg);
    if (!parsed.ok) {
      setFormError(parsed.error);
      return;
    }
    setFormError(null);
    actions.saveWeight(parsed.value);
  }

  function submitProfile(): void {
    const parsed = parseProfileInput(profileForm.heightCm, profileForm.sex);
    if (!parsed.ok) {
      setProfileError(parsed.error);
      return;
    }
    setProfileError(null);
    actions.saveProfile(parsed.value);
  }

  const latestWeight = state.entries[0]?.weightKg;

  return (
    <>
      <View style={[shared.section, styles.card]}>
        <Pressable
          accessibilityLabel="Edit profile"
          accessibilityRole="button"
          accessibilityState={{ expanded: profileOpen }}
          onPress={() => setProfileOpen((value) => !value)}
          style={({ pressed }) => [styles.rowHeader, pressed && shared.iconPressed]}
        >
          <Text style={shared.sectionTitle}>PROFILE</Text>
          <Text style={styles.formToggle}>{profileOpen ? "Hide" : "Edit"}</Text>
        </Pressable>
        {state.profile.tag === "loaded" ? (
          <Text style={styles.detail}>
            {[
              state.profile.profile.heightCm !== null
                ? `${state.profile.profile.heightCm} cm`
                : null,
              state.profile.profile.sex !== null ? state.profile.profile.sex : null,
            ]
              .filter(Boolean)
              .join(" · ") || "Not configured"}
          </Text>
        ) : null}
        {state.profile.tag === "loading" ? (
          <Text style={styles.detail}>Loading profile</Text>
        ) : null}
        {state.profile.tag === "error" ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.profile.message}
          </Text>
        ) : null}
        {profileOpen ? (
          <>
            <View style={shared.row}>
              <TextInput
                accessibilityLabel="Height in cm"
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="decimal-pad"
                onChangeText={(heightCm) => setProfileForm((current) => ({ ...current, heightCm }))}
                placeholder="Height cm"
                placeholderTextColor={colors.muted}
                style={[styles.input, styles.halfInput]}
                value={profileForm.heightCm ?? ""}
              />
              <TextInput
                accessibilityLabel="Sex"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={(sex) => setProfileForm((current) => ({ ...current, sex }))}
                placeholder="Sex, e.g. male"
                placeholderTextColor={colors.muted}
                style={[styles.input, styles.halfInput]}
                value={profileForm.sex ?? ""}
              />
            </View>
            <View style={shared.row}>
              <ActionButton
                label="Save profile"
                sending={state.saving}
                disabled={state.saving}
                onPress={submitProfile}
              />
            </View>
            {profileError !== null ? (
              <Text accessibilityRole="alert" style={shared.error}>
                {profileError}
              </Text>
            ) : null}
          </>
        ) : null}
      </View>

      <View style={[shared.section, styles.card]}>
        <Text style={shared.sectionTitle}>RECORD WEIGHT</Text>
        <View style={shared.row}>
          <TextInput
            accessibilityLabel="Entry date"
            autoCapitalize="none"
            autoCorrect={false}
            onChangeText={(date) => setForm((current) => ({ ...current, date }))}
            placeholder="YYYY-MM-DD"
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={form.date}
          />
          <TextInput
            accessibilityLabel="Body weight in kg"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="decimal-pad"
            onChangeText={(weightKg) => setForm((current) => ({ ...current, weightKg }))}
            placeholder={latestWeight === undefined ? "Weight kg" : `Last: ${latestWeight} kg`}
            placeholderTextColor={colors.muted}
            style={[styles.input, styles.halfInput]}
            value={form.weightKg}
          />
        </View>
        <View style={shared.row}>
          <ActionButton
            label="Record weight"
            sending={state.saving}
            disabled={state.saving}
            onPress={submitWeight}
          />
        </View>
        {formError !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {formError}
          </Text>
        ) : null}
        {state.saveError !== null ? (
          <Text accessibilityRole="alert" style={shared.error}>
            {state.saveError}
          </Text>
        ) : null}
      </View>

      {state.refreshing ? (
        <View style={shared.statusPanel}>
          <ActivityIndicator color={colors.signal} />
          <Text style={shared.statusText}>Refreshing</Text>
        </View>
      ) : null}
      {state.entries.length === 0 && !state.refreshing ? (
        <View style={shared.statusPanel}>
          <Text style={shared.statusText}>
            {state.error === null ? "No body weight entries yet" : state.error}
          </Text>
        </View>
      ) : null}
      {state.entries.map((entry) => (
        <View key={entry.id} style={shared.section}>
          <View style={shared.sectionHeader}>
            <Text style={styles.name}>{formatDate(entry.date)}</Text>
            <Text style={styles.detail}>{entry.weightKg} kg</Text>
          </View>
        </View>
      ))}
      <Pressable
        accessibilityRole="button"
        onPress={actions.refresh}
        style={({ pressed }) => [shared.retryButton, pressed && shared.actionPressed]}
      >
        <Text style={shared.retry}>Refresh entries</Text>
      </Pressable>
    </>
  );
}
