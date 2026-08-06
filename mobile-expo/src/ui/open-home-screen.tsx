import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View, type LayoutChangeEvent, type NativeScrollEvent, type NativeSyntheticEvent } from 'react-native';
import { useEffect, useRef, useState } from 'react';

import type { HomeGeofenceActions, HomeGeofenceState } from '../application/use-home-geofence';
import type { CommandState, IrState, OpenHomeActions, OpenHomeState, TopLevelTab } from '../application/use-open-home';
import { edifierRemoteRows, tvRemoteRows, type RemoteCommand } from '../domain/remotes';
import { ControlButton } from './control-button';
import { colors } from './theme';

type ReadyState = Extract<OpenHomeState, { readonly tag: 'ready' }>;

type Props = {
  readonly state: ReadyState;
  readonly actions: OpenHomeActions;
  readonly homeGeofence: HomeGeofenceState;
  readonly homeGeofenceActions: HomeGeofenceActions;
};

type IconName = 'home' | 'television' | 'speaker' | 'light' | 'away' | 'settings';

const tabs: ReadonlyArray<TopLevelTab> = ['home', 'television', 'speaker', 'away'];

const homeEdifierRows: ReadonlyArray<ReadonlyArray<RemoteCommand>> = [
  [
    { command: 'bluetooth', label: 'Bluetooth' },
    { command: 'optical', label: 'Optical' },
  ],
  [
    { command: 'volume-down', label: 'Volume -' },
    { command: 'volume-up', label: 'Volume +' },
  ],
];

const homeTelevisionRows: ReadonlyArray<ReadonlyArray<RemoteCommand>> = [[
  { command: 'power', label: 'Power' },
]];

/** Render the four primary OpenHome destinations. */
export function OpenHomeScreen({ state, actions, homeGeofence, homeGeofenceActions }: Props) {
  const pager = useRef<ScrollView>(null);
  const [pageWidth, setPageWidth] = useState(0);

  useEffect(() => {
    if (pageWidth > 0) {
      pager.current?.scrollTo({ x: tabIndex(state.selectedTab) * pageWidth, animated: false });
    }
  }, [pageWidth, state.selectedTab]);

  function selectTab(tab: TopLevelTab): void {
    actions.selectTab(tab);
    pager.current?.scrollTo({ x: tabIndex(tab) * pageWidth, animated: true });
  }

  function handleLayout(event: LayoutChangeEvent): void {
    setPageWidth(event.nativeEvent.layout.width);
  }

  function handleSwipe(event: NativeSyntheticEvent<NativeScrollEvent>): void {
    if (pageWidth === 0) {
      return;
    }
    const page = Math.max(0, Math.min(tabs.length - 1, Math.round(event.nativeEvent.contentOffset.x / pageWidth)));
    const tab = tabs[page];
    if (tab !== undefined && tab !== state.selectedTab) {
      actions.selectTab(tab);
    }
  }

  return (
    <View onLayout={handleLayout} style={styles.screen}>
      <View style={styles.header}>
        <View>
          <Text style={styles.kicker}>OPENHOME</Text>
          <View accessibilityLabel={connectionLabel(state.ir)} accessibilityRole="text" style={styles.connectionRow}>
            <View style={[styles.connectionDot, state.ir.tag === 'loaded' && styles.readyDot, state.ir.tag === 'error' && styles.errorDot]} />
            <Text style={styles.connection}>{connectionLabel(state.ir)}</Text>
          </View>
        </View>
        <Pressable
          accessibilityLabel="Configure OpenHome connection"
          accessibilityRole="button"
          hitSlop={8}
          onPress={actions.openReconfiguration}
          style={({ pressed }) => [styles.configureButton, pressed && styles.iconPressed]}
        >
          <Icon color={colors.muted} name="settings" size={20} />
        </Pressable>
      </View>

      <ScrollView
        horizontal
        onMomentumScrollEnd={handleSwipe}
        pagingEnabled
        ref={pager}
        showsHorizontalScrollIndicator={false}
        style={styles.content}
      >
        <Page width={pageWidth}>
          <HomePage state={state} actions={actions} homeGeofence={homeGeofence} homeGeofenceActions={homeGeofenceActions} />
        </Page>
        <Page width={pageWidth}><TelevisionPage state={state} actions={actions} /></Page>
        <Page width={pageWidth}><SpeakerPage state={state} actions={actions} /></Page>
        <Page width={pageWidth}><AwayPage state={homeGeofence} actions={homeGeofenceActions} /></Page>
      </ScrollView>

      <View accessibilityRole="tablist" style={styles.tabs}>
        <Tab icon="home" label="Home" selected={state.selectedTab === 'home'} onPress={() => selectTab('home')} />
        <Tab icon="television" label="TV" selected={state.selectedTab === 'television'} onPress={() => selectTab('television')} />
        <Tab icon="speaker" label="Speaker" selected={state.selectedTab === 'speaker'} onPress={() => selectTab('speaker')} />
        <Tab icon="away" label="Away" selected={state.selectedTab === 'away'} onPress={() => selectTab('away')} />
      </View>
    </View>
  );
}

function HomePage({ state, actions, homeGeofence, homeGeofenceActions }: Props) {
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="EVERYDAY CONTROL" title="Home" description="Your most-used controls, ready when you are." />
      {state.ir.tag !== 'loaded' ? <IrSummary state={state.ir} onRetry={actions.retryIrStatus} /> : null}

      <AutomationToggle state={homeGeofence} actions={homeGeofenceActions} />
      {homeGeofence.tag === 'ready' && homeGeofence.error !== null ? (
        <Text accessibilityRole="alert" style={styles.error}>{homeGeofence.error}</Text>
      ) : null}

      <HomeSpeakerSection>
        <RemoteRows
          rows={homeEdifierRows}
          available={state.ir.tag === 'loaded' ? state.ir.status.edifierCommands : new Set()}
          commands={state.edifier}
          onPress={actions.sendEdifierCommand}
        />
      </HomeSpeakerSection>

      <View style={styles.utilityGrid}>
        <CompactDeviceSection icon="television" title="TV">
          <RemoteRows
            rows={homeTelevisionRows}
            available={state.ir.tag === 'loaded' ? state.ir.status.lgTvCommands : new Set()}
            commands={state.television}
            onPress={actions.sendTelevisionCommand}
          />
        </CompactDeviceSection>
        <CompactDeviceSection icon="light" title="Lights">
          <View style={styles.row}>
            <CompactAction label="On" sending={state.light.sending.has('on')} disabled={state.light.sending.size > 0} onPress={() => actions.sendLightCommand('on')} />
            <CompactAction label="Off" sending={state.light.sending.has('off')} disabled={state.light.sending.size > 0} onPress={() => actions.sendLightCommand('off')} />
          </View>
          <CommandError state={state.light} commands={[{ command: 'on', label: 'Lights on' }, { command: 'off', label: 'Lights off' }]} />
        </CompactDeviceSection>
      </View>
    </View>
  );
}

function TelevisionPage({ state, actions }: Pick<Props, 'state' | 'actions'>) {
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="REMOTE" title="TV" description="Power, sound, channels, and navigation." />
      {state.ir.tag !== 'loaded' ? <IrSummary state={state.ir} onRetry={actions.retryIrStatus} /> : null}
      <DeviceSection icon="television" title="LG TV" detail="Full remote">
        <RemoteRows
          rows={tvRemoteRows}
          available={state.ir.tag === 'loaded' ? state.ir.status.lgTvCommands : new Set()}
          commands={state.television}
          onPress={actions.sendTelevisionCommand}
        />
      </DeviceSection>
    </View>
  );
}

function SpeakerPage({ state, actions }: Pick<Props, 'state' | 'actions'>) {
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="REMOTE" title="Speaker" description="Choose a source and shape the sound." />
      {state.ir.tag !== 'loaded' ? <IrSummary state={state.ir} onRetry={actions.retryIrStatus} /> : null}
      <DeviceSection icon="speaker" title="Edifier" detail="Full remote">
        <RemoteRows
          rows={edifierRemoteRows}
          available={state.ir.tag === 'loaded' ? state.ir.status.edifierCommands : new Set()}
          commands={state.edifier}
          onPress={actions.sendEdifierCommand}
        />
      </DeviceSection>
    </View>
  );
}

function Page({ width, children }: { readonly width: number; readonly children: React.ReactNode }) {
  return <ScrollView contentContainerStyle={styles.pageContent} style={{ width }}>{children}</ScrollView>;
}

function AwayPage({ state, actions }: { readonly state: HomeGeofenceState; readonly actions: HomeGeofenceActions }) {
  const enabled = state.tag === 'ready' && state.home !== null;
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="AUTOMATION" title="Leave home" description="Turn off the lights when this device leaves home." />
      <View style={styles.heroStatus}>
        {state.tag === 'loading' ? <ActivityIndicator color={colors.signal} /> : (
          <>
            <View style={[styles.statusIcon, enabled && styles.statusIconEnabled]}>
              <Icon color={enabled ? colors.ready : colors.muted} name="away" size={28} />
            </View>
            <View style={styles.statusCopy}>
              <Text style={styles.statusLabel}>
                {enabled ? `Automation on with ${providerLabel(state.home?.provider)}` : 'Automation off'}
              </Text>
              <Text style={styles.statusDescription}>
                {enabled
                  ? `Watching a ${state.home?.radiusMeters ?? state.radiusInput} meter radius around home.`
                  : 'Set your home location to enable the automation.'}
              </Text>
            </View>
          </>
        )}
      </View>

      <Section title="HOME REGION">
        <Text style={styles.automationDescription}>
          OpenHome uses your location only to detect when this device leaves the region. It then turns off your lights.
        </Text>
        <Text style={styles.inputLabel}>Home radius</Text>
        <View style={styles.radiusRow}>
          <TextInput
            accessibilityLabel="Home radius in meters"
            editable={state.tag === 'ready' && !state.saving}
            keyboardType="number-pad"
            onChangeText={actions.updateRadius}
            placeholder="150"
            placeholderTextColor={colors.muted}
            style={styles.radiusInput}
            value={state.radiusInput}
          />
          <Text style={styles.radiusUnit}>meters</Text>
        </View>
        {state.tag === 'loading' ? null : (
          <View style={styles.row}>
            <ActionButton label={enabled ? 'Update with Expo' : 'Set with Expo'} sending={state.saving} disabled={state.saving} onPress={() => actions.setHome('expo')} />
            <SecondaryAction label={enabled ? 'Update natively' : 'Set natively'} disabled={state.saving} onPress={() => actions.setHome('native')} />
          </View>
        )}
        {enabled ? <SecondaryAction label="Turn off automation" disabled={state.saving} onPress={actions.disable} /> : null}
        {state.tag === 'ready' && state.error !== null ? <Text accessibilityRole="alert" style={styles.error}>{state.error}</Text> : null}
      </Section>
    </View>
  );
}

function AutomationToggle({ state, actions }: { readonly state: HomeGeofenceState; readonly actions: HomeGeofenceActions }) {
  const enabled = state.tag === 'ready' && state.home !== null;
  const busy = state.tag === 'loading' || state.saving;

  function toggle(): void {
    if (state.tag !== 'ready' || state.saving) {
      return;
    }
    if (state.home === null) {
      actions.setHome(state.provider);
    } else {
      actions.disable();
    }
  }

  return (
    <Pressable
      accessibilityHint={enabled ? 'Turns off leave home automation' : `Sets this location as home using ${providerLabel(state.tag === 'ready' ? state.provider : undefined)} location monitoring`}
      accessibilityLabel="Leave home automation"
      accessibilityRole="switch"
      accessibilityState={{ checked: enabled, disabled: busy, busy }}
      disabled={busy}
      onPress={toggle}
      style={({ pressed }) => [styles.automationToggle, enabled && styles.automationToggleEnabled, busy && styles.disabled, pressed && styles.actionPressed]}
    >
      <View style={styles.automationToggleCopy}>
        <Text style={styles.automationToggleTitle}>Leave home</Text>
        <Text style={styles.compactDescription}>{automationToggleDescription(state)}</Text>
      </View>
      {busy ? <ActivityIndicator color={colors.signal} /> : (
        <View style={[styles.switchTrack, enabled && styles.switchTrackEnabled]}>
          <View style={[styles.switchThumb, enabled && styles.switchThumbEnabled]} />
        </View>
      )}
    </Pressable>
  );
}

function CompactDeviceSection({ icon, title, children }: { readonly icon: 'television' | 'light'; readonly title: string; readonly children: React.ReactNode }) {
  return (
    <View style={styles.compactSection}>
      <View style={styles.compactSectionHeader}>
        <Icon color={colors.signal} name={icon} size={19} />
        <Text style={styles.compactSectionTitle}>{title}</Text>
      </View>
      {children}
    </View>
  );
}

function HomeSpeakerSection({ children }: { readonly children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      <View style={styles.sectionHeader}>
        <View style={styles.deviceIdentity}>
          <Icon color={colors.signal} name="speaker" size={21} />
          <Text style={styles.deviceTitle}>Speaker</Text>
        </View>
        <Text style={styles.sectionDetail}>Audio</Text>
      </View>
      {children}
    </View>
  );
}

function DeviceSection({ icon, title, detail, children }: { readonly icon: 'speaker' | 'television'; readonly title: string; readonly detail: string; readonly children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      <View style={styles.sectionHeader}>
        <View style={styles.deviceIdentity}>
          <View style={styles.deviceIcon}><Icon color={colors.signal} name={icon} size={21} /></View>
          <Text style={styles.deviceTitle}>{title}</Text>
        </View>
        <Text style={styles.sectionDetail}>{detail}</Text>
      </View>
      {children}
    </View>
  );
}

function PageHeading({ eyebrow, title, description }: { readonly eyebrow: string; readonly title: string; readonly description: string }) {
  return (
    <View>
      <Text style={styles.pageEyebrow}>{eyebrow}</Text>
      <Text accessibilityRole="header" style={styles.pageTitle}>{title}</Text>
      <Text style={styles.pageDescription}>{description}</Text>
    </View>
  );
}

function Section({ title, children }: { readonly title: string; readonly children: React.ReactNode }) {
  return (
    <View style={styles.section}>
      <Text style={styles.sectionTitle}>{title}</Text>
      {children}
    </View>
  );
}

function IrSummary({ state, onRetry }: { readonly state: IrState; readonly onRetry: () => void }) {
  if (state.tag === 'loading') {
    return <View style={styles.statusPanel}><ActivityIndicator color={colors.signal} /><Text style={styles.statusText}>Checking remote</Text></View>;
  }
  if (state.tag === 'error') {
    return (
      <View style={styles.statusPanel}>
        <Text accessibilityRole="alert" style={styles.error}>{state.message}</Text>
        <Pressable accessibilityRole="button" onPress={onRetry} style={({ pressed }) => [styles.retryButton, pressed && styles.actionPressed]}>
          <Text style={styles.retry}>Try again</Text>
        </Pressable>
      </View>
    );
  }
  return null;
}

function RemoteRows({ rows, available, commands, onPress }: {
  readonly rows: ReadonlyArray<ReadonlyArray<RemoteCommand | null>>;
  readonly available: ReadonlySet<string>;
  readonly commands: CommandState;
  readonly onPress: (command: string) => void;
}) {
  return (
    <View style={styles.rows}>
      {rows.map((row, rowIndex) => (
        <View key={rowIndex} style={styles.row}>
          {row.map((control, columnIndex) => control === null
            ? <View key={`empty-${columnIndex}`} style={styles.spacer} />
            : <ControlButton key={control.command} control={control} available={available.has(control.command)} sending={commands.sending.has(control.command)} onPress={onPress} />)}
        </View>
      ))}
      <CommandError state={commands} commands={rows.flat().filter((command): command is RemoteCommand => command !== null)} />
    </View>
  );
}

function ActionButton({ label, sending, disabled, onPress }: { readonly label: string; readonly sending: boolean; readonly disabled: boolean; readonly onPress: () => void }) {
  return (
    <Pressable accessibilityRole="button" accessibilityState={{ disabled, busy: sending }} disabled={disabled} onPress={onPress} style={({ pressed }) => [styles.actionButton, disabled && styles.disabled, pressed && styles.actionPressed]}>
      {sending ? <ActivityIndicator color={colors.background} size="small" /> : null}
      <Text style={styles.actionLabel}>{label}</Text>
    </Pressable>
  );
}

function CompactAction({ label, sending, disabled, onPress }: { readonly label: string; readonly sending: boolean; readonly disabled: boolean; readonly onPress: () => void }) {
  return (
    <Pressable accessibilityLabel={`Turn lights ${label.toLowerCase()}`} accessibilityRole="button" accessibilityState={{ disabled, busy: sending }} disabled={disabled} onPress={onPress} style={({ pressed }) => [styles.compactAction, disabled && styles.disabled, pressed && styles.actionPressed]}>
      {sending ? <ActivityIndicator color={colors.text} size="small" /> : <Text style={styles.compactActionLabel}>{label}</Text>}
    </Pressable>
  );
}

function SecondaryAction({ label, disabled, onPress }: { readonly label: string; readonly disabled: boolean; readonly onPress: () => void }) {
  return (
    <Pressable accessibilityRole="button" accessibilityState={{ disabled }} disabled={disabled} onPress={onPress} style={({ pressed }) => [styles.secondaryAction, disabled && styles.disabled, pressed && styles.iconPressed]}>
      <Text style={styles.secondaryActionLabel}>{label}</Text>
    </Pressable>
  );
}

function CommandError({ state, commands }: { readonly state: CommandState; readonly commands: ReadonlyArray<RemoteCommand> }) {
  if (state.error === null) {
    return null;
  }
  const label = commands.find((command) => command.command === state.error?.command)?.label ?? state.error.command;
  return <Text accessibilityRole="alert" style={styles.error}>Unable to send {label}. {state.error.message}</Text>;
}

function Tab({ icon, label, selected, onPress }: { readonly icon: Exclude<IconName, 'settings'>; readonly label: string; readonly selected: boolean; readonly onPress: () => void }) {
  return (
    <Pressable accessibilityLabel={`${label} tab`} accessibilityRole="tab" accessibilityState={{ selected }} onPress={onPress} style={({ pressed }) => [styles.tab, pressed && styles.iconPressed]}>
      <View style={[styles.tabIndicator, selected && styles.selectedTabIndicator]} />
      <Icon color={selected ? colors.signal : colors.muted} name={icon} size={23} />
      <Text style={[styles.tabLabel, selected && styles.selectedTabLabel]}>{label}</Text>
    </Pressable>
  );
}

function Icon({ name, color, size }: { readonly name: IconName; readonly color: string; readonly size: number }) {
  if (name === 'home') {
    return <Text accessibilityElementsHidden importantForAccessibility="no" style={[styles.iconGlyph, { color, fontSize: size }]}>⌂</Text>;
  }
  if (name === 'television') {
    return <View accessibilityElementsHidden importantForAccessibility="no" style={[styles.tvIcon, { borderColor: color, height: size * 0.72, width: size }]}><View style={[styles.tvStand, { backgroundColor: color }]} /></View>;
  }
  if (name === 'speaker') {
    return <View accessibilityElementsHidden importantForAccessibility="no" style={[styles.speakerIcon, { borderColor: color, height: size, width: size * 0.72 }]}><View style={[styles.speakerDriver, { borderColor: color }]} /><View style={[styles.speakerTweeter, { backgroundColor: color }]} /></View>;
  }
  if (name === 'light') {
    return (
      <View accessibilityElementsHidden importantForAccessibility="no" style={[styles.bulbIcon, { height: size, width: size }]}>
        <View style={[styles.bulbGlow, { borderColor: color }]} />
        <View style={[styles.bulbNeck, { borderColor: color }]} />
        <View style={[styles.bulbBase, { backgroundColor: color }]} />
      </View>
    );
  }
  if (name === 'away') {
    return <Text accessibilityElementsHidden importantForAccessibility="no" style={[styles.iconGlyph, { color, fontSize: size }]}>↗</Text>;
  }
  return <Text accessibilityElementsHidden importantForAccessibility="no" style={[styles.iconGlyph, { color, fontSize: size }]}>⚙</Text>;
}

function connectionLabel(state: IrState): string {
  if (state.tag === 'loading') {
    return 'Connecting';
  }
  if (state.tag === 'error') {
    return 'Connection issue';
  }
  return 'Online';
}

function providerLabel(provider: 'expo' | 'native' | undefined): string {
  return provider === 'expo' ? 'Expo' : 'Native';
}

function automationToggleDescription(state: HomeGeofenceState): string {
  if (state.tag === 'loading') {
    return 'Checking automation';
  }
  if (state.home !== null) {
    return `On with ${providerLabel(state.home.provider)}`;
  }
  return `Turn off lights when you leave with ${providerLabel(state.provider)}`;
}

function tabIndex(tab: TopLevelTab): number {
  return tabs.indexOf(tab);
}

const styles = StyleSheet.create({
  actionButton: { alignItems: 'center', backgroundColor: colors.signal, borderRadius: 14, flex: 1, flexDirection: 'row', gap: 8, justifyContent: 'center', minHeight: 54, paddingHorizontal: 10 },
  actionLabel: { color: colors.background, fontSize: 14, fontWeight: '800', lineHeight: 19, textAlign: 'center' },
  actionPressed: { opacity: 0.84, transform: [{ scale: 0.96 }] },
  automationDescription: { color: colors.muted, fontSize: 15, lineHeight: 23 },
  automationToggle: { alignItems: 'center', backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 18, borderWidth: 1, flexDirection: 'row', gap: 14, justifyContent: 'space-between', minHeight: 74, paddingHorizontal: 16, paddingVertical: 12 },
  automationToggleCopy: { flex: 1, gap: 3 },
  automationToggleEnabled: { backgroundColor: colors.readyDark },
  automationToggleTitle: { color: colors.text, fontSize: 17, fontWeight: '800', letterSpacing: -0.2 },
  compactAction: { alignItems: 'center', backgroundColor: colors.panelRaised, borderColor: colors.border, borderRadius: 12, borderWidth: 1, flex: 1, justifyContent: 'center', minHeight: 48, paddingHorizontal: 8 },
  compactActionLabel: { color: colors.text, fontSize: 14, fontWeight: '700' },
  compactDescription: { color: colors.muted, fontSize: 13, lineHeight: 18 },
  compactSection: { backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 16, borderWidth: 1, flex: 1, gap: 12, minHeight: 126, padding: 14 },
  compactSectionHeader: { alignItems: 'center', flexDirection: 'row', gap: 8 },
  compactSectionTitle: { color: colors.text, fontSize: 15, fontWeight: '800' },
  configureButton: { alignItems: 'center', borderColor: colors.border, borderRadius: 12, borderWidth: 1, height: 44, justifyContent: 'center', width: 44 },
  connection: { color: colors.muted, fontSize: 12, fontWeight: '600', lineHeight: 16 },
  connectionDot: { backgroundColor: colors.muted, borderRadius: 4, height: 7, width: 7 },
  connectionRow: { alignItems: 'center', flexDirection: 'row', gap: 7 },
  content: { flex: 1 },
  deviceIcon: { alignItems: 'center', backgroundColor: colors.signalDark, borderRadius: 11, height: 38, justifyContent: 'center', width: 38 },
  deviceIdentity: { alignItems: 'center', flexDirection: 'row', gap: 10 },
  deviceTitle: { color: colors.text, fontSize: 18, fontWeight: '800', letterSpacing: -0.3 },
  disabled: { opacity: 0.48 },
  error: { color: colors.danger, fontSize: 13, lineHeight: 19, textAlign: 'center' },
  errorDot: { backgroundColor: colors.danger },
  header: { alignItems: 'center', flexDirection: 'row', justifyContent: 'space-between', paddingHorizontal: 20, paddingVertical: 14 },
  heroStatus: { alignItems: 'center', backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 20, borderWidth: 1, flexDirection: 'row', gap: 16, minHeight: 112, padding: 18 },
  iconGlyph: { fontWeight: '700', lineHeight: 28, textAlign: 'center' },
  iconPressed: { opacity: 0.72 },
  inputLabel: { color: colors.text, fontSize: 14, fontWeight: '700', marginTop: 4 },
  kicker: { color: colors.text, fontSize: 19, fontWeight: '900', letterSpacing: -0.7 },
  bulbBase: { borderRadius: 1, bottom: 1, height: 3, position: 'absolute', width: 8 },
  bulbGlow: { borderRadius: 8, borderWidth: 1.5, height: 13, position: 'absolute', top: 0, width: 13 },
  bulbIcon: { alignItems: 'center', position: 'relative' },
  bulbNeck: { borderBottomLeftRadius: 3, borderBottomRightRadius: 3, borderBottomWidth: 1.5, borderLeftWidth: 1.5, borderRightWidth: 1.5, bottom: 4, height: 6, position: 'absolute', width: 8 },
  pageContent: { padding: 20, paddingBottom: 32 },
  pageDescription: { color: colors.muted, fontSize: 16, lineHeight: 23, marginTop: 5 },
  pageEyebrow: { color: colors.signal, fontSize: 11, fontWeight: '800', letterSpacing: 1.6 },
  pageTitle: { color: colors.text, fontSize: 40, fontWeight: '900', letterSpacing: -1.6, lineHeight: 44, marginTop: 3 },
  radiusInput: { backgroundColor: colors.background, borderColor: colors.border, borderRadius: 12, borderWidth: 1, color: colors.text, flex: 1, fontSize: 16, minHeight: 50, paddingHorizontal: 14, paddingVertical: 12 },
  radiusRow: { alignItems: 'center', flexDirection: 'row', gap: 10 },
  radiusUnit: { color: colors.muted, fontSize: 14, fontWeight: '600' },
  readyDot: { backgroundColor: colors.ready },
  retry: { color: colors.signal, fontSize: 14, fontWeight: '800' },
  retryButton: { minHeight: 44, justifyContent: 'center', paddingHorizontal: 12 },
  row: { flexDirection: 'row', gap: 10 },
  rows: { gap: 10 },
  screen: { backgroundColor: colors.background, flex: 1 },
  secondaryAction: { alignItems: 'center', borderColor: colors.border, borderRadius: 14, borderWidth: 1, flex: 1, justifyContent: 'center', minHeight: 54, paddingHorizontal: 10 },
  secondaryActionLabel: { color: colors.text, fontSize: 14, fontWeight: '700', lineHeight: 19, textAlign: 'center' },
  section: { backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 20, borderWidth: 1, gap: 14, padding: 16 },
  sectionDetail: { color: colors.muted, fontSize: 12, fontWeight: '600' },
  sectionHeader: { alignItems: 'center', flexDirection: 'row', justifyContent: 'space-between' },
  sectionTitle: { color: colors.muted, fontSize: 11, fontWeight: '800', letterSpacing: 1.4 },
  selectedTabLabel: { color: colors.signal },
  selectedTabIndicator: { backgroundColor: colors.signal },
  speakerDriver: { borderRadius: 8, borderWidth: 1.5, bottom: 2.5, height: 8, position: 'absolute', width: 8 },
  speakerIcon: { borderRadius: 3, borderWidth: 1.5, position: 'relative' },
  speakerTweeter: { borderRadius: 2, height: 4, position: 'absolute', top: 3, width: 4 },
  spacer: { flex: 1 },
  stack: { gap: 18, width: '100%' },
  statusCopy: { flex: 1, gap: 4 },
  statusDescription: { color: colors.muted, fontSize: 14, lineHeight: 20 },
  statusIcon: { alignItems: 'center', backgroundColor: colors.panelRaised, borderRadius: 18, height: 58, justifyContent: 'center', width: 58 },
  statusIconEnabled: { backgroundColor: colors.readyDark },
  statusLabel: { color: colors.text, fontSize: 19, fontWeight: '800', letterSpacing: -0.3 },
  statusPanel: { alignItems: 'center', backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 16, borderWidth: 1, flexDirection: 'row', gap: 9, justifyContent: 'center', minHeight: 52, paddingHorizontal: 14, paddingVertical: 10 },
  statusText: { color: colors.text, fontSize: 14, fontWeight: '700' },
  switchThumb: { backgroundColor: colors.muted, borderRadius: 10, height: 20, transform: [{ translateX: 0 }], width: 20 },
  switchThumbEnabled: { backgroundColor: colors.background, transform: [{ translateX: 20 }] },
  switchTrack: { backgroundColor: colors.panelRaised, borderColor: colors.border, borderRadius: 14, borderWidth: 1, justifyContent: 'center', padding: 3, width: 48 },
  switchTrackEnabled: { backgroundColor: colors.ready, borderColor: colors.ready },
  tab: { alignItems: 'center', flex: 1, gap: 4, justifyContent: 'center', minHeight: 64, paddingHorizontal: 2, paddingVertical: 7, position: 'relative' },
  tabIndicator: { borderRadius: 2, height: 3, position: 'absolute', top: 0, width: 28 },
  tabLabel: { color: colors.muted, fontSize: 11, fontWeight: '700', lineHeight: 14 },
  tabs: { backgroundColor: colors.panel, borderTopColor: colors.border, borderTopWidth: 1, flexDirection: 'row' },
  tvIcon: { alignItems: 'center', borderRadius: 2, borderWidth: 1.5, justifyContent: 'flex-end' },
  tvStand: { bottom: -4, height: 1.5, position: 'absolute', width: 8 },
  utilityGrid: { flexDirection: 'row', gap: 10 },
});
