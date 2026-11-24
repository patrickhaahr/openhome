'use strict';

var assert = require('assert');
var fs = require('fs');
var path = require('path');
var vm = require('vm');

var source = fs.readFileSync(path.join(__dirname, 'ui/LauncherLifecycles.js'), 'utf8');
var lifecycles = vm.runInThisContext('(function () {' + source +
  '\nreturn {UNREAD: UNREAD, UNSENT: UNSENT, RELEASED: RELEASED, read: read, send: send, hold: hold}; })')();

var first = {};
var second = {};
var data = {day: '2026-10-21', weather: {state: 'fresh', temperature: 0}, recovery: {state: 'unavailable', charge: null}};
var offline = {day: '2026-10-21', weather: {state: 'stale', temperature: 0}, recovery: {state: 'unavailable', charge: null}};
var timeout = {type: 'timeout'};

function oneReadAtATime() {
  var reading = lifecycles.read(lifecycles.UNREAD, {type: 'start', flight: first});
  assert.deepStrictEqual(reading, {state: 'reading', flight: first, live: lifecycles.UNREAD.live});
  assert.strictEqual(lifecycles.read(reading, {type: 'start', flight: second}), reading);
  assert.strictEqual(lifecycles.read(reading, {type: 'settle', flight: second, data: data}), reading, 'only the current flight settles');
  var read = lifecycles.read(reading, {type: 'settle', flight: first, data: data});
  assert.deepStrictEqual(read, {state: 'idle', live: data});
  assert.strictEqual(lifecycles.read(read, {type: 'settle', flight: first, data: null}), read, 'a late callback is ignored');
  assert.strictEqual(lifecycles.read(read, timeout), read);
  var again = lifecycles.read(read, {type: 'start', flight: second});
  assert.deepStrictEqual(lifecycles.read(again, {type: 'settle', flight: second, data: null}), {state: 'idle', live: offline});
  var expired = lifecycles.read(again, timeout);
  assert.deepStrictEqual(expired, {state: 'idle', live: offline});
  assert.strictEqual(lifecycles.read(expired, {type: 'settle', flight: second, data: data}), expired, 'a late success is ignored');
  assert.strictEqual(data.weather.state, 'fresh', 'going offline copies the last reading');
}

function oneCommandAtATime() {
  var start = {type: 'start', flight: first, command: 'lights-on'};
  var other = {type: 'start', flight: second, command: 'lights-off'};
  var sending = lifecycles.send(lifecycles.UNSENT, start);
  assert.deepStrictEqual(sending, {state: 'sending', flight: first, command: 'lights-on'});
  assert.strictEqual(lifecycles.send(sending, other), sending);
  assert.strictEqual(lifecycles.send(sending, {type: 'settle', flight: second, accepted: true}), sending, 'only the current flight settles');
  assert.strictEqual(lifecycles.send(sending, {type: 'feedbackElapsed'}), sending);
  var accepted = lifecycles.send(sending, {type: 'settle', flight: first, accepted: true});
  assert.deepStrictEqual(accepted, {state: 'accepted', command: 'lights-on'}, 'an acknowledgement carries no device state');
  assert.strictEqual(lifecycles.send(accepted, {type: 'feedbackElapsed'}), lifecycles.UNSENT, 'and expires');
  var uncertain = lifecycles.send(sending, {type: 'settle', flight: first, accepted: false});
  assert.deepStrictEqual(uncertain, {state: 'uncertain', command: 'lights-on'});
  assert.deepStrictEqual(lifecycles.send(sending, timeout), uncertain, 'an unanswered command may have been sent');
  assert.strictEqual(lifecycles.send(uncertain, {type: 'feedbackElapsed'}), uncertain, 'uncertainty lasts until the next command');
  [accepted, uncertain, lifecycles.UNSENT].forEach(function (settled) {
    assert.strictEqual(lifecycles.send(settled, {type: 'settle', flight: first, accepted: true}), settled, 'a late callback is ignored');
    assert.strictEqual(lifecycles.send(settled, timeout), settled);
    assert.strictEqual(lifecycles.send(settled, other).command, 'lights-off');
  });
}

var idle = {state: 'idle'};
var pressed = {state: 'pressed'};
var held = {state: 'held'};
var tapping = {state: 'releasing', from: 'pressed'};
var lifting = {state: 'releasing', from: 'held'};

function holdTransitions() {
  [
    [idle, 'press', pressed, ['startHoldTimer']],
    [pressed, 'press', pressed, []],
    [held, 'press', held, []],
    [tapping, 'press', pressed, ['stopReleaseTimer']],
    [lifting, 'press', held, ['stopReleaseTimer']],
    [idle, 'release', idle, []],
    [pressed, 'release', tapping, ['startReleaseTimer']],
    [held, 'release', lifting, ['startReleaseTimer']],
    [tapping, 'release', tapping, ['startReleaseTimer']],
    [lifting, 'release', lifting, ['startReleaseTimer']],
    [pressed, 'holdElapsed', held, []],
    [tapping, 'holdElapsed', lifting, []],
    [tapping, 'releaseElapsed', idle, ['stopTimers', 'activate']],
    [lifting, 'releaseElapsed', idle, ['stopTimers', 'activate']]
  ].forEach(function (row) {
    assert.deepStrictEqual(lifecycles.hold(row[0], {type: row[1], editable: false}), {press: row[2], effects: row[3]}, JSON.stringify(row));
  });
  [pressed, tapping].forEach(function (press) {
    assert.deepStrictEqual(lifecycles.hold(press, {type: 'holdElapsed', editable: true}), {press: idle, effects: ['stopTimers', 'edit']});
  });
  [idle, pressed, held, tapping, lifting].forEach(function (press) {
    assert.deepStrictEqual(lifecycles.hold(press, {type: 'cancel'}), {press: idle, effects: ['stopTimers']});
  });
}

function lateTimersAreIgnored() {
  [idle, held, lifting].forEach(function (press) {
    assert.deepStrictEqual(lifecycles.hold(press, {type: 'holdElapsed', editable: true}), {press: press, effects: []});
  });
  [idle, pressed, held].forEach(function (press) {
    assert.deepStrictEqual(lifecycles.hold(press, {type: 'releaseElapsed'}), {press: press, effects: []});
  });
}

oneReadAtATime();
oneCommandAtATime();
holdTransitions();
lateTimersAreIgnored();
console.log('TV launcher: single read, single command, acknowledgement, OK-key hold and late callbacks passed');
