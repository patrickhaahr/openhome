'use strict';

var assert = require('assert');
var fs = require('fs');
var path = require('path');
var vm = require('vm');

var source = fs.readFileSync(path.join(__dirname, 'ui/PictureProfileLifecycle.js'), 'utf8');
var lifecycle = vm.runInThisContext('(function () {' + source +
  '\nreturn {UNKNOWN: UNKNOWN, pending: pending, transition: transition, request: request, dispatch: dispatch, parse: parse}; })')();

function run(profile, events) { return events.reduce(lifecycle.transition, profile); }
function reply(token, mode) { return {type: 'reply', token: token, accepted: true, mode: mode}; }
function rejected(token) { return {type: 'reply', token: token, accepted: false}; }
function sent(token) { return {type: 'sent', token: token}; }
var refresh = {type: 'refresh'};
var toggle = {type: 'toggle'};
var timeout = {type: 'timeout'};
var cinema = {state: 'idle', value: false, failed: false};
var failed = {state: 'idle', value: null, failed: true};

var reading = run(lifecycle.UNKNOWN, [refresh, sent(1)]);
var toggling = run(cinema, [toggle, sent(2)]);
var writing = run(toggling, [reply(2, 'cinema'), sent(3)]);
var verifying = run(writing, [reply(3), sent(4)]);
var operations = [reading, toggling, writing, verifying];

function view(profile) { return {state: profile.state, value: profile.value, failed: profile.failed}; }

function toggleReadsWritesAndReadsBack() {
  assert.deepStrictEqual(operations.map(function (profile) { return profile.state; }), ['reading', 'toggling', 'writing', 'verifying']);
  assert.deepStrictEqual(view(run(reading, [reply(1, 'cinema')])), cinema);
  assert.strictEqual(writing.target, 'expert2');
  assert.strictEqual(run(toggling, [reply(2, 'expert2')]).target, 'cinema', 'the target follows the mode just read, not the shown value');
  assert.deepStrictEqual(view(run(verifying, [reply(4, 'expert2')])), {state: 'idle', value: true, failed: false});
  assert.deepStrictEqual(lifecycle.request(run(toggling, [reply(2, 'cinema')])),
    {method: '/setSystemSettings', payload: {category: 'picture', settings: {pictureMode: 'expert2'}}});
  assert.deepStrictEqual(lifecycle.request(run(writing, [reply(3)])),
    {method: '/getSystemSettings', payload: {category: 'picture', keys: ['pictureMode']}});
}

function acknowledgementIsNotDeviceState() {
  assert.strictEqual(writing.value, false);
  assert.strictEqual(verifying.value, false, 'an accepted write leaves the observed value alone');
  assert.deepStrictEqual(view(run(writing, [reply(3, 'expert2')])), {state: 'verifying', value: false, failed: false});
  assert.deepStrictEqual(view(run(verifying, [reply(4, 'cinema')])), {state: 'idle', value: false, failed: true}, 'a mismatch keeps the observed mode');
}

function oneFlightAtATime() {
  [toggling, writing, verifying].forEach(function (profile) {
    assert.strictEqual(lifecycle.transition(profile, refresh), profile);
    assert.strictEqual(lifecycle.transition(profile, toggle), profile);
    assert.strictEqual(lifecycle.transition(profile, {type: 'observed', mode: 'expert2'}), profile);
  });
  operations.forEach(function (profile) {
    assert.strictEqual(lifecycle.request(profile), null, 'a sent request is never sent again');
    assert.strictEqual(lifecycle.transition(profile, sent(99)), profile);
  });
  var superseded = run(reading, [refresh, sent(5)]);
  assert.strictEqual(lifecycle.transition(superseded, reply(1, 'expert2')), superseded, 'a newer read replaces the flight');
  var preempted = run(cinema, [refresh, sent(6), toggle, sent(7)]);
  assert.strictEqual(preempted.state, 'toggling');
  assert.strictEqual(lifecycle.transition(preempted, rejected(6)), preempted);
  assert.strictEqual(lifecycle.transition(lifecycle.UNKNOWN, toggle), lifecycle.UNKNOWN, 'an unknown mode cannot be toggled');
}

function wire() {
  var calls = [];
  var outstanding = [];
  var setting = {profile: lifecycle.UNKNOWN};
  var timer = {running: false, stop: function () { timer.running = false; }, restart: function () { timer.running = true; }};
  var service = {
    call: function (uri, method) {
      assert.deepStrictEqual(outstanding, [], 'the previous request is gone before the next is sent');
      assert.strictEqual(setting.profile.token, null, 'the previous token is invalid before the next request is sent');
      var token = calls.push(method);
      outstanding.push(token);
      return token;
    },
    cancel: function (token) {
      assert(outstanding.indexOf(token) !== -1, 'only an outstanding request is cancelled');
      assert.notStrictEqual(setting.profile.token, token, 'a token is invalid before its request is cancelled');
      outstanding.splice(outstanding.indexOf(token), 1);
    }
  };
  function dispatch(event) { lifecycle.dispatch(setting, event, service, timer); }
  function answer(token, mode) {
    outstanding = outstanding.filter(function (open) { return open !== token; });
    dispatch(reply(token, mode));
  }
  return {setting: setting, calls: calls, dispatch: dispatch, answer: answer,
    live: function () { return {outstanding: outstanding.slice(), timer: timer.running}; }};
}

function oneRequestIsOutstandingAtTheService() {
  var luna = wire();
  luna.dispatch(refresh);
  luna.dispatch(refresh);
  assert.deepStrictEqual(luna.live(), {outstanding: [2], timer: true}, 'a newer read cancels the one it replaces');
  luna.answer(1, 'expert2');
  assert.deepStrictEqual(view(luna.setting.profile), {state: 'reading', value: null, failed: false});
  luna.answer(2, 'cinema');
  assert.deepStrictEqual(view(luna.setting.profile), cinema);
  assert.deepStrictEqual(luna.live(), {outstanding: [], timer: false});

  luna.dispatch(refresh);
  luna.dispatch(toggle);
  assert.deepStrictEqual(luna.live().outstanding, [4], 'a toggle cancels the read it replaces');
  luna.dispatch(refresh);
  luna.dispatch(toggle);
  luna.answer(4, 'cinema');
  luna.answer(5);
  luna.dispatch({type: 'observed', mode: 'cinema'});
  assert.deepStrictEqual(luna.live().outstanding, [6]);
  luna.answer(6, 'expert2');
  assert.deepStrictEqual(luna.calls.slice(3), ['/getSystemSettings', '/setSystemSettings', '/getSystemSettings']);
  assert.deepStrictEqual(view(luna.setting.profile), {state: 'idle', value: true, failed: false});
  assert.deepStrictEqual(luna.live(), {outstanding: [], timer: false});

  luna.dispatch(refresh);
  luna.dispatch({type: 'observed', mode: 'cinema'});
  assert.deepStrictEqual(luna.live().outstanding, [7], 'an observation leaves the read in flight');
  luna.dispatch(timeout);
  assert.deepStrictEqual(luna.live(), {outstanding: [], timer: false}, 'a timed-out request is cancelled');
  luna.dispatch(refresh);
  assert.deepStrictEqual(luna.live().outstanding, [8]);
}

function everyOperationHasAnExit() {
  operations.forEach(function (profile) {
    assert.deepStrictEqual(view(lifecycle.transition(profile, timeout)), failed);
    assert.deepStrictEqual(view(lifecycle.transition(profile, rejected(profile.token))), failed);
  });
  [reading, toggling, verifying].forEach(function (profile) {
    ['', null, undefined, 7].forEach(function (mode) {
      assert.deepStrictEqual(view(lifecycle.transition(profile, reply(profile.token, mode))), failed);
    });
  });
  assert.deepStrictEqual(view(run(failed, [refresh, sent(8), reply(8, 'cinema')])), cinema, 'a failure recovers on the next read');
}

function lateCallbacksAreIgnored() {
  operations.forEach(function (profile) {
    var settled = lifecycle.transition(profile, timeout);
    assert.strictEqual(lifecycle.transition(settled, reply(profile.token, 'expert2')), settled);
    assert.strictEqual(lifecycle.transition(settled, timeout), settled);
    assert.strictEqual(lifecycle.transition(profile, reply(99, 'expert2')), profile);
  });
  assert.strictEqual(lifecycle.transition(verifying, reply(2, 'cinema')), verifying);
}

function observationsFollowTheRemote() {
  var observed = {type: 'observed', mode: 'expert2'};
  assert.deepStrictEqual(view(lifecycle.transition(failed, observed)), {state: 'idle', value: true, failed: false});
  var updated = lifecycle.transition(reading, observed);
  assert.deepStrictEqual([updated.state, updated.token, updated.value], ['reading', 1, true]);
  ['', undefined].forEach(function (mode) {
    assert.strictEqual(lifecycle.transition(cinema, {type: 'observed', mode: mode}), cinema);
  });
}

function repliesAreParsed() {
  assert.deepStrictEqual(lifecycle.parse('{"returnValue":true,"settings":{"pictureMode":"cinema"}}'), {accepted: true, mode: 'cinema'});
  assert.deepStrictEqual(lifecycle.parse('{"returnValue":true}'), {accepted: true, mode: undefined});
  ['{"returnValue":false}', '{', 'null', ''].forEach(function (payload) {
    assert.strictEqual(lifecycle.parse(payload).accepted, false);
  });
}

toggleReadsWritesAndReadsBack();
acknowledgementIsNotDeviceState();
oneFlightAtATime();
oneRequestIsOutstandingAtTheService();
everyOperationHasAnExit();
lateCallbacksAreIgnored();
observationsFollowTheRemote();
repliesAreParsed();
console.log('TV picture profile: toggle, readback, single flight, exits and late callbacks passed');
