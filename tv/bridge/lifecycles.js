'use strict';

var RETRY_AFTER = 60 * 1000;
var COMMAND_INTERVAL = 1000;

function unseen(day) { return {state: 'unavailable', day: day, dueAt: 0}; }

function observe(observation, event) {
  switch (event.type) {
    case 'tick':
      if (event.day !== observation.day || event.at >= observation.discardAt) return unseen(event.day);
      return observation.state === 'fresh' && event.at >= observation.staleAt ?
        Object.assign({}, observation, {state: 'stale'}) : observation;
    case 'requested':
      return Object.assign({}, observation, {dueAt: event.at + RETRY_AFTER});
    case 'received':
      if (event.day !== observation.day) return observation;
      return {state: event.reading.state, day: observation.day, reading: event.reading,
        dueAt: event.dueAt === undefined ? observation.dueAt : event.dueAt,
        staleAt: event.staleAt, discardAt: event.discardAt};
    case 'missed':
      return event.day === observation.day && observation.state === 'fresh' ?
        Object.assign({}, observation, {state: 'stale'}) : observation;
  }
  return observation;
}

function refresh(lifecycle, event) {
  if (event.type === 'start' && lifecycle.state === 'idle') return {state: 'refreshing', flight: event.flight};
  if (event.type === 'settle' && lifecycle.state === 'refreshing' && event.flight === lifecycle.flight) return {state: 'idle'};
  return lifecycle;
}

function admit(admission, event) {
  if (event.type === 'send' && admission.state === 'idle' && event.at >= admission.nextAt) {
    return {state: 'sending', flight: event.flight, nextAt: event.at + COMMAND_INTERVAL};
  }
  if (event.type === 'settle' && admission.state === 'sending' && event.flight === admission.flight) {
    return {state: 'idle', nextAt: admission.nextAt};
  }
  return admission;
}

module.exports = {RETRY_AFTER: RETRY_AFTER, unseen: unseen, observe: observe, refresh: refresh, admit: admit};
