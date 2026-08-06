# Expo HAS CHANGED

Read the exact versioned docs at https://docs.expo.dev/versions/v57.0.0/ before writing any code.

## Project Shape

- `src/domain/` contains pure parsing and control definitions.
- `src/application/` owns the application state machine and sequences effects.
- `src/infrastructure/` contains SecureStore and Axum API adapters.
- `src/ui/` contains render-only React Native components.
- Mobile clients call only the Axum API. Do not add direct device or bridge integrations.

## Commands

- `just expo-start`
- `just expo-android`
- `just expo-check`
