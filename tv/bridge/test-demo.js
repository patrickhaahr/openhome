'use strict';

var assert = require('assert');
var client = require('./client');
var demoBridge = require('./demo');
var now = Date.parse('2026-12-02T09:30:00Z');

function demo() {
  function score(value) { return {value: value, unit: 'score_0_100', source: 'my-whoop'}; }
  return {capturedOn: '2026-11-28', recovery: {
    day: '2026-11-28', time_zone: 'Europe/Copenhagen', sleep: {private: 'not forwarded'},
    derived_scores: {recovery: score(72), rest: score(90.22), strain: score(41.5)},
    noop: {freshness: 'partial', last_push_at: '2026-11-28T01:57:33+01:00', installation_id: 'not forwarded'}
  }};
}

function shifts() {
  assert.strictEqual(demoBridge.wholeDays('2026-11-28', '2026-12-02'), 4);
  assert.strictEqual(demoBridge.wholeDays('2026-03-20', '2026-03-31'), 11, 'a DST change does not leave a fractional day');
  assert.strictEqual(demoBridge.shiftInstant('2026-11-28T01:57:33+01:00', 4), '2026-12-02T00:57:33.000Z');
  assert.strictEqual(demoBridge.shiftInstant('2026-11-28T01:57:33+01:00', -160), '2026-06-20T23:57:33.000Z',
    'the Copenhagen wall clock survives a shift into summer time');
  assert.strictEqual(demoBridge.shiftInstant('2026-06-20T01:57:33+02:00', 161), '2026-11-28T00:57:33.000Z',
    'the Copenhagen wall clock survives a shift out of summer time');
  assert.strictEqual(demoBridge.shiftInstant('2026-11-28T00:00:00+01:00', 1), '2026-11-28T23:00:00.000Z');
  assert.strictEqual(demoBridge.shiftInstant(null, 4), null);
}

async function answers() {
  var forwarded = [];
  var request = demoBridge.createRequest({client: client, demo: demo(), now: function () { return now; },
    forward: function (path, method) {
      forwarded.push(method + ' ' + path);
      return Promise.resolve({forwarded: path});
    }});
  var body = await request('/api/training/days/2026-12-02/recovery', 'GET', null, 1000);
  assert.deepStrictEqual(client.recoveryFrom(body, '2026-12-02'), {state: 'fresh', charge: 72, rest: 90.22, effort: 41.5,
    freshness: 'partial', updatedAt: '2026-12-02T00:57:33.000Z'}, 'the capture day is answered as today');
  assert.strictEqual(JSON.stringify(body).indexOf('not forwarded'), -1, 'only the fields the bridge reads are kept');
  assert.deepStrictEqual(await request('/api/weather', 'GET', null, 1000), {forwarded: '/api/weather'});
  assert.deepStrictEqual(await request('/api/training/days/2026-12-01/recovery', 'GET', null, 1000),
    {forwarded: '/api/training/days/2026-12-01/recovery'}, 'other days stay real');
  await request('/api/training/days/2026-12-02/recovery', 'POST', {}, 1000);
  await request('/api/lights/on', 'POST', null, 1000);
  assert.deepStrictEqual(forwarded, ['GET /api/weather', 'GET /api/training/days/2026-12-01/recovery',
    'POST /api/training/days/2026-12-02/recovery', 'POST /api/lights/on'], 'weather, other days and every non-GET stay real');
}

async function throughTheBridge() {
  var bridge = client.createBridge({now: function () { return now; }, request: demoBridge.createRequest({
    client: client, demo: demo(), now: function () { return now; },
    forward: function () { return Promise.reject(new Error('real API unreachable')); }
  })});
  var state = await bridge.getState();
  assert.strictEqual(state.recovery.state, 'fresh');
  assert.strictEqual(state.recovery.charge, 72);
  assert.strictEqual(state.weather.state, 'unavailable', 'a failed real request still degrades the way it does live');
}

module.exports = async function () {
  shifts();
  await answers();
  await throughTheBridge();
};
