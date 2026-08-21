import { StatusBar } from "expo-status-bar";
import { ActivityIndicator, AppState, StyleSheet, View } from "react-native";
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context";
import { useEffect } from "react";

import { useHomeGeofence } from "./src/application/use-home-geofence";
import { useOpenHome } from "./src/application/use-open-home";
import { createSecureConfigurationStore } from "./src/infrastructure/configuration-store";
import { createExpoHomeGeofenceBackend } from "./src/infrastructure/expo-home-geofence-backend";
import { createHomeGeofenceService } from "./src/infrastructure/home-geofence-service";
import { createHomeGeofenceStore } from "./src/infrastructure/home-geofence-store";
import { retryPendingHomeExitCommand } from "./src/infrastructure/home-geofence-task";
import { createNativeHomeGeofenceBackend } from "./src/infrastructure/native-home-geofence-backend";
import { OpenHomeScreen } from "./src/ui/open-home-screen";
import { ConfigurationScreen } from "./src/ui/configuration-screen";
import { colors } from "./src/ui/theme";

const configurationStore = createSecureConfigurationStore();
const homeGeofenceService = createHomeGeofenceService(createHomeGeofenceStore(), {
  expo: createExpoHomeGeofenceBackend(),
  native: createNativeHomeGeofenceBackend(),
});

export default function App() {
  const [state, actions] = useOpenHome(configurationStore);
  const [homeGeofence, homeGeofenceActions] = useHomeGeofence(homeGeofenceService);

  useEffect(() => {
    void homeGeofenceService.resume();
    void retryPendingHomeExitCommand();
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (nextState === "active") {
        void homeGeofenceService.resume();
        void retryPendingHomeExitCommand();
      }
    });
    return () => subscription.remove();
  }, []);

  return (
    <SafeAreaProvider>
      <SafeAreaView edges={["top", "right", "bottom", "left"]} style={styles.container}>
        {state.tag === "loading" ? (
          <View style={styles.loading}>
            <ActivityIndicator color={colors.signal} size="large" />
          </View>
        ) : null}
        {state.tag === "configuration" ? (
          <ConfigurationScreen state={state} actions={actions} />
        ) : null}
        {state.tag === "ready" ? (
          <OpenHomeScreen
            state={state}
            actions={actions}
            homeGeofence={homeGeofence}
            homeGeofenceActions={homeGeofenceActions}
          />
        ) : null}
        <StatusBar style="light" />
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.background,
  },
  loading: {
    alignItems: "center",
    flex: 1,
    justifyContent: "center",
  },
});
