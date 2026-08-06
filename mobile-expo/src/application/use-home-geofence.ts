import { useEffect, useReducer, useRef } from 'react';

import { parseRadiusMeters, type HomeGeofence, type HomeGeofenceProvider } from '../domain/home-geofence';
import type { HomeGeofenceService } from '../infrastructure/home-geofence-service';

/** User-visible home-geofence state. */
export type HomeGeofenceState =
  | { readonly tag: 'loading'; readonly radiusInput: string }
  | {
      readonly tag: 'ready';
      readonly home: HomeGeofence | null;
      readonly radiusInput: string;
      readonly saving: boolean;
      readonly error: string | null;
    };

/** User actions accepted by the home-geofence flow. */
export type HomeGeofenceActions = {
  readonly updateRadius: (value: string) => void;
  readonly setHome: (provider: HomeGeofenceProvider) => void;
  readonly disable: () => void;
};

type Event =
  | { readonly type: 'loaded'; readonly home: HomeGeofence | null; readonly error: string | null }
  | { readonly type: 'radiusChanged'; readonly value: string }
  | { readonly type: 'saving' }
  | { readonly type: 'saved'; readonly home: HomeGeofence }
  | { readonly type: 'disabled' }
  | { readonly type: 'failed'; readonly message: string };

/** Coordinate home-geofence setup and removal for the OpenHome UI. */
export function useHomeGeofence(service: HomeGeofenceService): readonly [HomeGeofenceState, HomeGeofenceActions] {
  const [state, dispatch] = useReducer(reduce, { tag: 'loading', radiusInput: '150' });
  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    let active = true;
    void service.load().then((result) => {
      if (active) {
        dispatch(result.ok
          ? { type: 'loaded', home: result.value, error: null }
          : { type: 'loaded', home: null, error: result.error });
      }
    });
    return () => { active = false; };
  }, [service]);

  async function setHome(provider: HomeGeofenceProvider): Promise<void> {
    const current = stateRef.current;
    if (current.tag !== 'ready' || current.saving) {
      return;
    }
    const radius = parseRadiusMeters(current.radiusInput);
    if (!radius.ok) {
      dispatch({ type: 'failed', message: radius.error });
      return;
    }
    dispatch({ type: 'saving' });
    const result = await service.setAtCurrentLocation(radius.value, provider);
    dispatch(result.ok ? { type: 'saved', home: result.value } : { type: 'failed', message: result.error });
  }

  async function disable(): Promise<void> {
    const current = stateRef.current;
    if (current.tag !== 'ready' || current.saving || current.home === null) {
      return;
    }
    dispatch({ type: 'saving' });
    const result = await service.disable();
    dispatch(result.ok ? { type: 'disabled' } : { type: 'failed', message: result.error });
  }

  return [state, {
    updateRadius(value) { dispatch({ type: 'radiusChanged', value }); },
    setHome(provider) { void setHome(provider); },
    disable() { void disable(); },
  }] as const;
}

function reduce(state: HomeGeofenceState, event: Event): HomeGeofenceState {
  switch (event.type) {
    case 'loaded':
      return {
        tag: 'ready',
        home: event.home,
        radiusInput: event.home?.radiusMeters.toString() ?? state.radiusInput,
        saving: false,
        error: event.error,
      };
    case 'radiusChanged':
      if (state.tag === 'loading') {
        return { ...state, radiusInput: event.value };
      }
      return { ...state, radiusInput: event.value, error: null };
    case 'saving':
      return state.tag === 'ready' ? { ...state, saving: true, error: null } : state;
    case 'saved':
      return state.tag === 'ready' ? { ...state, home: event.home, radiusInput: event.home.radiusMeters.toString(), saving: false, error: null } : state;
    case 'disabled':
      return state.tag === 'ready' ? { ...state, home: null, saving: false, error: null } : state;
    case 'failed':
      return state.tag === 'ready' ? { ...state, saving: false, error: event.message } : state;
  }
}
