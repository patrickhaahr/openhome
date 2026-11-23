'use strict';

var assert = require('assert');
var lifecycles = require('./lifecycles');

var DAY = '2026-10-21';
var unseen = lifecycles.unseen(DAY);
var received = {type: 'received', day: DAY, reading: {state: 'fresh', temperature: 0}, dueAt: 200, staleAt: 200, discardAt: 500};
var fresh = lifecycles.observe(unseen, received);
var stale = Object.assign({}, fresh, {state: 'stale'});

function oneRefreshAtATime() {
  var first = {};
  var second = {};
  var refreshing = lifecycles.refresh({state: 'idle'}, {type: 'start', flight: first});
  assert.deepStrictEqual(refreshing, {state: 'refreshing', flight: first});
  assert.strictEqual(lifecycles.refresh(refreshing, {type: 'start', flight: second}), refreshing);
  assert.strictEqual(lifecycles.refresh(refreshing, {type: 'settle', flight: second}), refreshing, 'only the current flight settles');
  var idle = lifecycles.refresh(refreshing, {type: 'settle', flight: first});
  assert.deepStrictEqual(idle, {state: 'idle'});
  assert.strictEqual(lifecycles.refresh(idle, {type: 'settle', flight: first}), idle, 'a late settlement is ignored');
}

function oneCommandAtATime() {
  var first = {};
  var second = {};
  var idle = {state: 'idle', nextAt: 0};
  var sending = lifecycles.admit(idle, {type: 'send', at: 100, flight: first});
  assert.deepStrictEqual(sending, {state: 'sending', flight: first, nextAt: 1100});
  assert.strictEqual(lifecycles.admit(sending, {type: 'send', at: 5000, flight: second}), sending, 'elapsed time never replaces a flight');
  assert.strictEqual(lifecycles.admit(sending, {type: 'settle', flight: second}), sending, 'only the current flight settles');
  var cooling = lifecycles.admit(sending, {type: 'settle', flight: first});
  assert.deepStrictEqual(cooling, {state: 'idle', nextAt: 1100}, 'settling never releases the start deadline');
  assert.strictEqual(lifecycles.admit(cooling, {type: 'settle', flight: first}), cooling, 'a late settlement is ignored');
  assert.strictEqual(lifecycles.admit(cooling, {type: 'send', at: 1099, flight: second}), cooling);
  assert.strictEqual(lifecycles.admit(cooling, {type: 'send', at: 1100, flight: second}).state, 'sending');
}

function observationsExpire() {
  function tick(observation, at, day) { return lifecycles.observe(observation, {type: 'tick', at: at, day: day || DAY}); }
  assert.strictEqual(tick(fresh, 199), fresh);
  assert.deepStrictEqual(tick(fresh, 200), stale);
  assert.strictEqual(tick(stale, 499), stale);
  assert.deepStrictEqual(tick(fresh, 500), unseen, 'discarding outranks going stale');
  assert.deepStrictEqual(tick(stale, 500), unseen);
  assert.deepStrictEqual(tick(fresh, 0, '2026-10-22'), lifecycles.unseen('2026-10-22'));
  assert.deepStrictEqual(tick(stale, 0, '2026-10-22'), lifecycles.unseen('2026-10-22'));
  var undated = lifecycles.observe(unseen, {type: 'received', day: DAY, reading: {state: 'fresh'}});
  assert.strictEqual(tick(undated, Infinity), undated, 'a reading without its own deadlines lasts the day');
}

function observationsSettle() {
  function missed(observation, day) { return lifecycles.observe(observation, {type: 'missed', day: day || DAY}); }
  var requested = lifecycles.observe(unseen, {type: 'requested', at: 100});
  assert.deepStrictEqual(requested, {state: 'unavailable', day: DAY, dueAt: 100 + lifecycles.RETRY_AFTER});
  assert.strictEqual(missed(requested), requested, 'a miss keeps the retry deadline and invents no reading');
  assert.deepStrictEqual(missed(fresh), stale);
  assert.strictEqual(missed(stale), stale);
  assert.strictEqual(lifecycles.observe(requested, {type: 'received', day: DAY, reading: {state: 'fresh'}}).dueAt, requested.dueAt);
  assert.deepStrictEqual(lifecycles.observe(stale, received), fresh);
  var tomorrow = lifecycles.unseen('2026-10-22');
  assert.strictEqual(lifecycles.observe(tomorrow, received), tomorrow, 'a result for an earlier day is ignored');
  assert.strictEqual(missed(fresh, '2026-10-20'), fresh, 'a failure for another day is ignored');
}

module.exports = function () {
  oneRefreshAtATime();
  oneCommandAtATime();
  observationsExpire();
  observationsSettle();
};
