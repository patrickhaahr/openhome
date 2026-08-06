import { describe, expect, it } from 'vitest';

import { parseIrStatus } from './open-home-api';

describe('parseIrStatus', () => {
  it('parses and deduplicates each remote command set', () => {
    const result = parseIrStatus({
      message: 'IR ready',
      remotes: {
        edifier: ['bluetooth', 'bluetooth', ' optical '],
        lgtv: ['power'],
      },
    });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.message).toBe('IR ready');
      expect([...result.value.edifierCommands]).toEqual(['bluetooth', 'optical']);
      expect([...result.value.lgTvCommands]).toEqual(['power']);
    }
  });

  it('rejects malformed remote command lists', () => {
    expect(parseIrStatus({ remotes: { edifier: ['power'], lgtv: 'power' } })).toEqual({
      ok: false,
      error: "Couldn't read IR status from the Axum API.",
    });
  });
});
