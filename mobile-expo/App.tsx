import { StatusBar } from "expo-status-bar";
import { ActivityIndicator, AppState, Linking, StyleSheet, View } from "react-native";
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context";
import { useEffect } from "react";

import { useAdGuard } from "./src/application/use-adguard";
import { useDocker } from "./src/application/use-docker";
import { useFeeds } from "./src/application/use-feeds";
import { useHomeGeofence } from "./src/application/use-home-geofence";
import { useOpenHome } from "./src/application/use-open-home";
import { useTimeline } from "./src/application/use-timeline";
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
  const [state, actions, api] = useOpenHome(configurationStore);
  const [homeGeofence, homeGeofenceActions] = useHomeGeofence(homeGeofenceService);
  const [adguard, adguardActions] = useAdGuard(api === null ? null : api.adguard);
  const [docker, dockerActions, dockerCounts] = useDocker(api === null ? null : api.docker);
  const [timeline, timelineActions] = useTimeline(api === null ? null : api.rss);
  const [feeds, feedsActions] = useFeeds(
    api === null ? null : api.rss,
    timelineActions.refresh,
  );

  useEffect(() => {
    void homeGeofenceService.resume();
    void retryPendingHomeExitCommand();
    const subscription = AppState.addEventListener("change", (nextState) => {
      if (nextState === "active") {
        void homeGeofenceService.resume();
        void retryPendingHomeExitCommand();
        adguardActions.refresh();
        dockerActions.refresh();
        timelineActions.refresh();
        feedsActions.refresh();
      }
    });
    return () => subscription.remove();
  }, [homeGeofenceService, adguardActions, dockerActions, timelineActions, feedsActions]);

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
            adguard={adguard}
            adguardActions={adguardActions}
            docker={docker}
            dockerActions={dockerActions}
            dockerCounts={dockerCounts}
            timeline={timeline}
            timelineActions={timelineActions}
            feeds={feeds}
            feedsActions={feedsActions}
            onOpenTimelineLink={(url) => {
              void Linking.openURL(url).catch(() => {});
            }}
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
