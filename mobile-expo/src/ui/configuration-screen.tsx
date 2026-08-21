import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

import type { OpenHomeActions, OpenHomeState } from "../application/use-open-home";
import { colors } from "./theme";

type ConfigurationState = Extract<OpenHomeState, { readonly tag: "configuration" }>;

type Props = {
  readonly state: ConfigurationState;
  readonly actions: OpenHomeActions;
};

/** Render first-run setup and safe configuration replacement. */
export function ConfigurationScreen({ state, actions }: Props) {
  const isReconfigure = state.mode === "reconfigure";
  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === "ios" ? "padding" : undefined}
      style={styles.screen}
    >
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        <View style={styles.identity}>
          <Text style={styles.kicker}>PRIVATE CONTROL PLANE</Text>
          <Text style={styles.wordmark}>OPEN{`\n`}HOME</Text>
          <View style={styles.signalLine} />
        </View>

        <View style={styles.panel}>
          <Text style={styles.title}>
            {isReconfigure ? "Update connection" : "Connect your home"}
          </Text>
          <Text style={styles.description}>
            {isReconfigure
              ? "Your current connection stays active until the replacement passes its health check."
              : "Enter the address and API key for your OpenHome server."}
          </Text>
          <Text style={styles.label}>BASE URL</Text>
          <TextInput
            autoCapitalize="none"
            autoCorrect={false}
            editable={!state.isSaving}
            keyboardType="url"
            onChangeText={actions.updateBaseUrl}
            placeholder="http://192.168.1.20:8000"
            placeholderTextColor={colors.muted}
            style={styles.input}
            value={state.baseUrl}
          />
          <Text style={styles.label}>API KEY</Text>
          <TextInput
            autoCapitalize="none"
            autoCorrect={false}
            editable={!state.isSaving}
            onChangeText={actions.updateApiKey}
            placeholder="Your bearer key"
            placeholderTextColor={colors.muted}
            secureTextEntry
            style={styles.input}
            value={state.apiKey}
          />
          {state.error === null ? null : (
            <Text accessibilityRole="alert" style={styles.error}>
              {state.error}
            </Text>
          )}
          <View style={styles.actions}>
            {isReconfigure ? (
              <Pressable
                disabled={state.isSaving}
                onPress={actions.cancelReconfiguration}
                style={styles.secondaryButton}
              >
                <Text style={styles.secondaryLabel}>Cancel</Text>
              </Pressable>
            ) : null}
            <Pressable
              disabled={state.isSaving}
              onPress={actions.submitConfiguration}
              style={styles.primaryButton}
            >
              {state.isSaving ? <ActivityIndicator color={colors.background} /> : null}
              <Text style={styles.primaryLabel}>
                {state.isSaving
                  ? "Checking connection"
                  : isReconfigure
                    ? "Save connection"
                    : "Connect"}
              </Text>
            </Pressable>
          </View>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  actions: { flexDirection: "row", gap: 12, marginTop: 8 },
  content: { flexGrow: 1, justifyContent: "center", padding: 24 },
  description: { color: colors.muted, fontSize: 15, lineHeight: 22, marginBottom: 10 },
  error: { color: colors.danger, fontSize: 14, lineHeight: 20 },
  identity: { marginBottom: 32 },
  input: {
    backgroundColor: colors.background,
    borderColor: colors.border,
    borderRadius: 10,
    borderWidth: 1,
    color: colors.text,
    fontSize: 16,
    paddingHorizontal: 14,
    paddingVertical: 13,
  },
  kicker: { color: colors.signal, fontSize: 11, fontWeight: "800", letterSpacing: 2 },
  label: { color: colors.muted, fontSize: 10, fontWeight: "800", letterSpacing: 1.5, marginTop: 6 },
  panel: {
    alignSelf: "center",
    backgroundColor: colors.panel,
    borderColor: colors.border,
    borderRadius: 18,
    borderWidth: 1,
    gap: 10,
    maxWidth: 520,
    padding: 22,
    width: "100%",
  },
  primaryButton: {
    alignItems: "center",
    backgroundColor: colors.signal,
    borderRadius: 10,
    flex: 1,
    flexDirection: "row",
    gap: 8,
    justifyContent: "center",
    minHeight: 50,
    paddingHorizontal: 14,
  },
  primaryLabel: { color: colors.background, fontSize: 14, fontWeight: "800" },
  screen: { backgroundColor: colors.background, flex: 1 },
  secondaryButton: {
    alignItems: "center",
    borderColor: colors.border,
    borderRadius: 10,
    borderWidth: 1,
    flex: 1,
    justifyContent: "center",
    minHeight: 50,
  },
  secondaryLabel: { color: colors.text, fontSize: 14, fontWeight: "700" },
  signalLine: { backgroundColor: colors.signal, height: 3, marginTop: 18, width: 48 },
  title: { color: colors.text, fontSize: 24, fontWeight: "800", letterSpacing: -0.5 },
  wordmark: {
    color: colors.text,
    fontSize: 48,
    fontWeight: "900",
    letterSpacing: -2,
    lineHeight: 43,
    marginTop: 8,
  },
});
