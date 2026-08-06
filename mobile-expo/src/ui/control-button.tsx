import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';

import type { RemoteCommand } from '../domain/remotes';
import { colors } from './theme';

type Props = {
  readonly control: RemoteCommand;
  readonly available: boolean;
  readonly sending: boolean;
  readonly onPress: (command: string) => void;
};

/** Render one command while keeping unavailable controls visible. */
export function ControlButton({ control, available, sending, onPress }: Props) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={control.label}
      accessibilityState={{ disabled: !available || sending, busy: sending }}
      disabled={!available || sending}
      onPress={() => onPress(control.command)}
      style={({ pressed }) => [styles.button, !available && styles.unavailable, pressed && styles.pressed]}
    >
      <View style={styles.content}>
        {sending ? <ActivityIndicator color={colors.signal} size="small" /> : null}
        <Text style={[styles.label, !available && styles.unavailableLabel]}>{control.label}</Text>
        {!available ? <Text style={styles.status}>Unavailable</Text> : null}
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  button: {
    alignItems: 'center',
    backgroundColor: colors.panelRaised,
    borderColor: colors.border,
    borderRadius: 14,
    borderWidth: 1,
    flex: 1,
    justifyContent: 'center',
    minHeight: 62,
    paddingHorizontal: 6,
    paddingVertical: 10,
  },
  content: { alignItems: 'center', gap: 3 },
  label: { color: colors.text, fontSize: 14, fontWeight: '700', lineHeight: 19, textAlign: 'center' },
  pressed: { backgroundColor: colors.signalDark, borderColor: colors.signal, transform: [{ scale: 0.96 }] },
  status: { color: colors.muted, fontSize: 11, fontWeight: '600', lineHeight: 15 },
  unavailable: { backgroundColor: colors.panel, opacity: 0.56 },
  unavailableLabel: { color: colors.muted },
});
