'use strict';

var assert = require('assert');
var client = require('./client');
var MINUTE = 60000;
var now = Date.parse('2026-10-21T12:00:00Z');

function nextTurn() { return new Promise(setImmediate); }
function rejected() { return 'rejected'; }

async function failedProcessingRetriesAtItsDeadline(weather, recovery) {
  var clock = now;
  var clockFails = false;
  var calls = [];
  var bridge = client.createBridge({now: function () {
    if (clockFails) { clockFails = false; throw new Error('synthetic clock failure'); }
    return clock;
  }, request: function (path) {
    calls.push(path);
    if (path !== '/api/weather') return Promise.resolve(recovery());
    clockFails = calls.length === 1;
    return Promise.resolve(weather());
  }});
  assert.strictEqual(await bridge.getState().catch(rejected), 'rejected', 'a refresh that fails while processing rejects its readers');
  assert.strictEqual((await bridge.getState()).recovery.rest, 82.5, 'the other request still lands and the slot is released');
  assert.strictEqual(calls.length, 2);
  clock = now + MINUTE - 1;
  assert.strictEqual((await bridge.getState()).weather.state, 'unavailable');
  assert.strictEqual(calls.length, 2, 'reads before the retry deadline make no upstream request');
  clock = now + MINUTE;
  assert.strictEqual((await bridge.getState()).weather.state, 'fresh');
  assert.deepStrictEqual(calls.slice(2), ['/api/weather', '/api/training/days/2026-10-21/recovery']);
}

async function failedProcessingHoldsTheSlotUntilItsSiblingSettles(weather, recovery) {
  var clock = now;
  var clockFails = false;
  var charges = [22, 82];
  var sent = 0;
  var pending = [];
  var mostPending = 0;
  var bridge = client.createBridge({now: function () {
    if (clockFails) { clockFails = false; throw new Error('synthetic clock failure'); }
    return clock;
  }, request: function (path) {
    if (path === '/api/weather') { clockFails = sent === 0; return Promise.resolve(weather()); }
    var body = recovery();
    body.derived_scores.recovery.value = charges[sent++];
    return new Promise(function (resolve) {
      pending.push(function () { resolve(body); });
      mostPending = Math.max(mostPending, pending.length);
    });
  }});
  async function answerNewestFirst() {
    while (pending.length) { pending.pop()(); await nextTurn(); }
  }
  var first = bridge.getState().catch(rejected);
  await nextTurn();
  clock += MINUTE;
  var overlapping = bridge.getState().catch(rejected);
  await nextTurn();
  await answerNewestFirst();
  assert.strictEqual(await first, 'rejected');
  await overlapping;
  var latest = bridge.getState();
  await nextTurn();
  await answerNewestFirst();
  assert.strictEqual((await latest).recovery.charge, 82, 'an older reply never overwrites a newer one');
  assert.strictEqual(mostPending, 1, 'the slot is held until every request in the refresh settles');
  assert.strictEqual(sent, 2);
}

module.exports = async function (weather, recovery) {
  await failedProcessingRetriesAtItsDeadline(weather, recovery);
  await failedProcessingHoldsTheSlotUntilItsSiblingSettles(weather, recovery);
};
