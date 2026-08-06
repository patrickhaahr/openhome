import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, View, type LayoutChangeEvent, type NativeScrollEvent, type NativeSyntheticEvent } from 'react-native';
import { useEffect, useRef, useState } from 'react';

import type { CommandState, IrState, OpenHomeActions, OpenHomeState, TopLevelTab } from '../application/use-open-home';
import { homeRemoteRows, tvRemoteRows, type RemoteCommand } from '../domain/remotes';
import { ControlButton } from './control-button';
import { colors } from './theme';

type ReadyState = Extract<OpenHomeState, { readonly tag: 'ready' }>;

type Props = {
  readonly state: ReadyState;
  readonly actions: OpenHomeActions;
};

/** Render Home and Remote as a swipeable, two-tab control console. */
export function ConfiguredScreen({ state, actions }: Props) {
  const pager = useRef<ScrollView>(null);
  const [pageWidth, setPageWidth] = useState(0);

  useEffect(() => {
    if (pageWidth > 0) {
      pager.current?.scrollTo({ x: state.selectedTab === 'home' ? 0 : pageWidth, animated: false });
    }
  }, [pageWidth, state.selectedTab]);

  function selectTab(tab: TopLevelTab): void {
    actions.selectTab(tab);
    pager.current?.scrollTo({ x: tab === 'home' ? 0 : pageWidth, animated: true });
  }

  function handleLayout(event: LayoutChangeEvent): void {
    setPageWidth(event.nativeEvent.layout.width);
  }

  function handleSwipe(event: NativeSyntheticEvent<NativeScrollEvent>): void {
    if (pageWidth === 0) {
      return;
    }
    actions.selectTab(event.nativeEvent.contentOffset.x >= pageWidth / 2 ? 'remote' : 'home');
  }

  return (
    <View onLayout={handleLayout} style={styles.screen}>
      <View style={styles.header}>
        <View>
          <Text style={styles.kicker}>OPENHOME</Text>
          <View style={styles.connectionRow}>
            <View style={styles.readyDot} />
            <Text style={styles.connection}>CONTROL PLANE ONLINE</Text>
          </View>
        </View>
        <Pressable accessibilityRole="button" onPress={actions.openReconfiguration} style={styles.configureButton}>
          <Text style={styles.configureLabel}>CONFIGURE</Text>
        </Pressable>
      </View>

      <ScrollView
        horizontal
        onMomentumScrollEnd={handleSwipe}
        pagingEnabled
        ref={pager}
        showsHorizontalScrollIndicator={false}
        style={styles.pager}
      >
        <ScrollView contentContainerStyle={styles.pageContent} style={{ width: pageWidth }}>
          <HomePage state={state} actions={actions} />
        </ScrollView>
        <ScrollView contentContainerStyle={styles.pageContent} style={{ width: pageWidth }}>
          <RemotePage state={state} actions={actions} />
        </ScrollView>
      </ScrollView>

      <View style={styles.tabs}>
        <Tab label="HOME" selected={state.selectedTab === 'home'} onPress={() => selectTab('home')} />
        <Tab label="REMOTE" selected={state.selectedTab === 'remote'} onPress={() => selectTab('remote')} />
      </View>
    </View>
  );
}

function HomePage({ state, actions }: Props) {
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="QUICK CONTROL" title="Home" description="The controls you reach for first." />
      <Section title="LIGHTS">
        <View style={styles.row}>
          <ActionButton label="Light on" sending={state.light.sending.has('on')} disabled={state.light.sending.size > 0} onPress={() => actions.sendLightCommand('on')} />
          <ActionButton label="Light off" sending={state.light.sending.has('off')} disabled={state.light.sending.size > 0} onPress={() => actions.sendLightCommand('off')} />
        </View>
        <CommandError state={state.light} commands={[{ command: 'on', label: 'Light on' }, { command: 'off', label: 'Light off' }]} />
      </Section>
      <IrSummary state={state.ir} remote="EDIFIER" onRetry={actions.retryIrStatus} />
      <Section title="EDIFIER">
        <RemoteRows
          rows={homeRemoteRows}
          available={state.ir.tag === 'loaded' ? state.ir.status.edifierCommands : new Set()}
          commands={state.edifier}
          onPress={actions.sendEdifierCommand}
        />
      </Section>
    </View>
  );
}

function RemotePage({ state, actions }: Props) {
  return (
    <View style={styles.stack}>
      <PageHeading eyebrow="IR REMOTE" title="LG TV" description="A fixed layout. Live availability." />
      <IrSummary state={state.ir} remote="LG TV" onRetry={actions.retryIrStatus} />
      <Section title="REMOTE">
        <RemoteRows
          rows={tvRemoteRows}
          available={state.ir.tag === 'loaded' ? state.ir.status.lgTvCommands : new Set()}
          commands={state.television}
          onPress={actions.sendTelevisionCommand}
        />
      </Section>
    </View>
  );
}

function PageHeading({ eyebrow, title, description }: { readonly eyebrow: string; readonly title: string; readonly description: string }) {
  return (
    <View>
      <Text style={styles.pageEyebrow}>{eyebrow}</Text>
      <Text style={styles.pageTitle}>{title}</Text>
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

function IrSummary({ state, remote, onRetry }: { readonly state: IrState; readonly remote: string; readonly onRetry: () => void }) {
  if (state.tag === 'loading') {
    return <View style={styles.statusPanel}><ActivityIndicator color={colors.signal} /><Text style={styles.statusText}>Loading IR status</Text></View>;
  }
  if (state.tag === 'error') {
    return (
      <View style={styles.statusPanel}>
        <Text accessibilityRole="alert" style={styles.error}>{state.message}</Text>
        <Pressable accessibilityRole="button" onPress={onRetry}><Text style={styles.retry}>RETRY IR STATUS</Text></Pressable>
      </View>
    );
  }
  const count = remote === 'EDIFIER' ? state.status.edifierCommands.size : state.status.lgTvCommands.size;
  return (
    <View style={styles.statusPanel}>
      <View style={styles.connectionRow}><View style={styles.readyDot} /><Text style={styles.statusText}>{state.status.message}</Text></View>
      <Text style={styles.commandCount}>{count} {remote} COMMANDS AVAILABLE</Text>
    </View>
  );
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

function CommandError({ state, commands }: { readonly state: CommandState; readonly commands: ReadonlyArray<RemoteCommand> }) {
  if (state.error === null) {
    return null;
  }
  const label = commands.find((command) => command.command === state.error?.command)?.label ?? state.error.command;
  return <Text accessibilityRole="alert" style={styles.error}>{label} failed: {state.error.message}</Text>;
}

function Tab({ label, selected, onPress }: { readonly label: string; readonly selected: boolean; readonly onPress: () => void }) {
  return (
    <Pressable accessibilityRole="tab" accessibilityState={{ selected }} onPress={onPress} style={styles.tab}>
      <Text style={[styles.tabLabel, selected && styles.selectedTabLabel]}>{label}</Text>
      <View style={[styles.tabLine, selected && styles.selectedTabLine]} />
    </Pressable>
  );
}

const styles = StyleSheet.create({
  actionButton: { alignItems: 'center', backgroundColor: colors.signal, borderRadius: 12, flex: 1, flexDirection: 'row', gap: 8, justifyContent: 'center', minHeight: 58 },
  actionLabel: { color: colors.background, fontSize: 15, fontWeight: '900' },
  actionPressed: { opacity: 0.8 },
  commandCount: { color: colors.muted, fontSize: 10, fontWeight: '800', letterSpacing: 1 },
  configureButton: { borderColor: colors.border, borderRadius: 8, borderWidth: 1, paddingHorizontal: 12, paddingVertical: 9 },
  configureLabel: { color: colors.muted, fontSize: 10, fontWeight: '800', letterSpacing: 1 },
  connection: { color: colors.muted, fontSize: 9, fontWeight: '800', letterSpacing: 1 },
  connectionRow: { alignItems: 'center', flexDirection: 'row', gap: 7 },
  disabled: { opacity: 0.5 },
  error: { color: colors.danger, fontSize: 13, lineHeight: 19, textAlign: 'center' },
  header: { alignItems: 'center', borderBottomColor: colors.border, borderBottomWidth: 1, flexDirection: 'row', justifyContent: 'space-between', paddingHorizontal: 20, paddingVertical: 14 },
  kicker: { color: colors.text, fontSize: 18, fontWeight: '900', letterSpacing: -0.5 },
  pageContent: { padding: 20, paddingBottom: 36 },
  pageDescription: { color: colors.muted, fontSize: 15, marginTop: 3 },
  pageEyebrow: { color: colors.signal, fontSize: 10, fontWeight: '900', letterSpacing: 2 },
  pageTitle: { color: colors.text, fontSize: 38, fontWeight: '900', letterSpacing: -1.5, marginTop: 2 },
  pager: { flex: 1 },
  readyDot: { backgroundColor: colors.ready, borderRadius: 4, height: 7, width: 7 },
  retry: { color: colors.signal, fontSize: 11, fontWeight: '900', letterSpacing: 1, textAlign: 'center' },
  row: { flexDirection: 'row', gap: 10 },
  rows: { gap: 10 },
  screen: { backgroundColor: colors.background, flex: 1 },
  section: { backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 16, borderWidth: 1, gap: 12, padding: 14 },
  sectionTitle: { color: colors.muted, fontSize: 10, fontWeight: '900', letterSpacing: 1.8 },
  selectedTabLabel: { color: colors.signal },
  selectedTabLine: { backgroundColor: colors.signal },
  spacer: { flex: 1 },
  stack: { gap: 18, width: '100%' },
  statusPanel: { alignItems: 'center', backgroundColor: colors.panel, borderColor: colors.border, borderRadius: 12, borderWidth: 1, gap: 8, padding: 14 },
  statusText: { color: colors.text, fontSize: 14, fontWeight: '700' },
  tab: { alignItems: 'center', flex: 1, gap: 9, paddingTop: 15 },
  tabLabel: { color: colors.muted, fontSize: 11, fontWeight: '900', letterSpacing: 1.5 },
  tabLine: { height: 3, width: 44 },
  tabs: { backgroundColor: colors.panel, borderTopColor: colors.border, borderTopWidth: 1, flexDirection: 'row', minHeight: 58 },
});
