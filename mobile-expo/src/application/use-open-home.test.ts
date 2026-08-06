import { describe, expect, it } from 'vitest';

import { reduce, type OpenHomeState } from './use-open-home';

const configuration = { baseUrl: 'http://openhome.local:8000', apiKey: 'secret' };

describe('OpenHome state transitions', () => {
  it('opens and cancels reconfiguration without replacing the active configuration', () => {
    const loaded = reduce({ tag: 'loading' }, { type: 'configurationLoaded', configuration, error: null });
    const ready = reduce(loaded, {
      type: 'irLoaded',
      status: { message: 'IR ready', edifierCommands: new Set(['power']), lgTvCommands: new Set(['power']) },
    });
    const television = reduce(ready, { type: 'tabSelected', tab: 'television' });
    const editing = reduce(television, { type: 'reconfigurationOpened' });
    const changed = reduce(editing, { type: 'baseUrlChanged', value: 'http://replacement.local' });
    const cancelled = reduce(changed, { type: 'reconfigurationCancelled' });

    expect(cancelled.tag).toBe('ready');
    if (cancelled.tag === 'ready') {
      expect(cancelled.configuration).toEqual(configuration);
      expect(cancelled.selectedTab).toBe('television');
      expect(cancelled.ir.tag).toBe('loaded');
    }
  });

  it('tracks concurrent commands independently', () => {
    const ready = reduce({ tag: 'loading' }, { type: 'configurationLoaded', configuration, error: null });
    const first = reduce(ready, { type: 'commandStarted', target: 'edifier', command: 'power' });
    const second = reduce(first, { type: 'commandStarted', target: 'television', command: 'power' });

    expect(commandSet(second, 'edifier')).toEqual(['power']);
    expect(commandSet(second, 'television')).toEqual(['power']);
  });

  it("does not clear one command's error when another command succeeds", () => {
    const ready = reduce({ tag: 'loading' }, { type: 'configurationLoaded', configuration, error: null });
    const sending = reduce(reduce(ready, { type: 'commandStarted', target: 'edifier', command: 'power' }), {
      type: 'commandStarted', target: 'edifier', command: 'mute',
    });
    const failed = reduce(sending, { type: 'commandFinished', target: 'edifier', command: 'power', error: 'Bridge offline' });
    const succeeded = reduce(failed, { type: 'commandFinished', target: 'edifier', command: 'mute', error: null });

    expect(succeeded.tag === 'ready' ? succeeded.edifier.error : null).toEqual({ command: 'power', message: 'Bridge offline' });
  });
});

function commandSet(state: OpenHomeState, target: 'edifier' | 'television'): ReadonlyArray<string> {
  return state.tag === 'ready' ? [...state[target].sending] : [];
}
