'use strict';

var assert = require('assert');
var http = require('http');
var https = require('https');
var events = require('events');
var fs = require('fs');
var os = require('os');
var path = require('path');
var client = require('./client');
var MINUTE = 60000;
var now = Date.parse('2026-10-21T12:00:00Z');

function weather() {
  return {state: 'fresh', forecast: {
    fetched_at: '2026-10-21T12:00:00Z', valid_until: '2026-10-21T12:15:00Z', timezone: 'Europe/Copenhagen',
    private_coordinate: 'must never leave the bridge',
    current: {observed_at: '2026-10-21T12:00:00Z', temperature_c: 0, weather_code: 0, wind_ms: 0},
    days: [
      {date: '2026-10-20', weather_code: 61, high_c: 13, low_c: 8, precipitation_mm: 12, wind_ms: 7},
      {date: '2026-10-21', weather_code: 0, high_c: 4, low_c: -2, precipitation_mm: 0, wind_ms: 0}
    ]
  }};
}

function recovery() {
  function score(value) { return {value: value, unit: 'score_0_100', source: 'private-device'}; }
  return {day: '2026-10-21', time_zone: 'Europe/Copenhagen', derived_scores: {
    recovery: score(0), rest: score(82.5), strain: score(null)
  }, journal: [{notes: 'private journal'}], noop: {
    freshness: 'partial', last_push_at: '2026-10-21T11:56:00Z', installation_id: 'private-installation'
  }};
}

async function transforms() {
  assert.strictEqual(client.dayOn(Date.parse('2026-10-24T22:30:00Z')), '2026-10-25');
  assert.strictEqual(client.dayOn(Date.parse('2026-10-25T23:30:00Z')), '2026-10-26');
  assert.strictEqual(client.dayOn(Date.parse('2026-03-28T23:30:00Z')), '2026-03-29');
  assert.strictEqual(client.dayOn(Date.parse('2026-03-29T22:30:00Z')), '2026-03-30');
  var normal = client.weatherFrom(weather(), '2026-10-21', now);
  assert.deepStrictEqual(normal, {state: 'fresh', temperature: 0, code: 0, high: 4, low: -2,
    rain: 0, wind: 0, fetchedAt: '2026-10-21T12:00:00Z', validUntil: '2026-10-21T12:15:00Z'});
  assert.strictEqual(client.weatherFrom(weather(), '2026-10-22', now), null);
  assert.strictEqual(client.weatherFrom(weather(), '2026-10-21', now + 15 * MINUTE).state, 'stale');
  assert.strictEqual(client.weatherFrom(weather(), '2026-10-21', now + 6 * 60 * MINUTE), null);
  var missing = weather();
  missing.forecast.current = null;
  missing.forecast.days[1].high_c = null;
  assert.strictEqual(client.weatherFrom(missing, '2026-10-21', now).temperature, null);
  assert.strictEqual(client.weatherFrom(missing, '2026-10-21', now).high, null);
  var malformed = weather();
  malformed.forecast.current.temperature_c = '0';
  assert.strictEqual(client.weatherFrom(malformed, '2026-10-21', now), null);
  malformed = weather();
  delete malformed.forecast.current.weather_code;
  assert.strictEqual(client.weatherFrom(malformed, '2026-10-21', now), null);
  var scores = client.recoveryFrom(recovery(), '2026-10-21');
  assert.deepStrictEqual(scores, {state: 'fresh', charge: 0, rest: 82.5, effort: null,
    freshness: 'partial', updatedAt: '2026-10-21T11:56:00Z'});
  assert.strictEqual(client.recoveryFrom(recovery(), '2026-10-22'), null);
  malformed = recovery();
  malformed.derived_scores.rest.value = 101;
  assert.strictEqual(client.recoveryFrom(malformed, '2026-10-21'), null);
  malformed = recovery();
  malformed.derived_scores.rest.unit = 'min';
  assert.strictEqual(client.recoveryFrom(malformed, '2026-10-21'), null);
  malformed = recovery();
  malformed.noop.freshness = 'synced';
  assert.strictEqual(client.recoveryFrom(malformed, '2026-10-21'), null);
}

async function configBoundary() {
  var directory = fs.mkdtempSync(path.join(os.tmpdir(), 'openhome-tv-test-'));
  var file = path.join(directory, 'config.json');
  var link = path.join(directory, 'link.json');
  var config = {apiKey: 'synthetic-key', transportHost: '100.120.132.42', serverName: 'openhome.haahr.me'};
  try {
    fs.writeFileSync(file, JSON.stringify(config), {mode: 0o600});
    assert.deepStrictEqual(client.loadConfig(file), config);
    fs.chmodSync(file, 0o644);
    assert.throws(function () { client.loadConfig(file); }, /owner-only/);
    fs.chmodSync(file, 0o600);
    fs.symlinkSync(file, link);
    assert.throws(function () { client.loadConfig(link); }, /owner-only/);
    config.serverName = 'attacker.invalid';
    fs.writeFileSync(file, JSON.stringify(config));
    assert.throws(function () { client.loadConfig(file); }, /Invalid TV client configuration/);
    config.serverName = 'openhome.haahr.me';
    config.apiKey = 'invalid\r\nheader';
    fs.writeFileSync(file, JSON.stringify(config));
    assert.throws(function () { client.loadConfig(file); }, /Invalid TV client configuration/);
  } finally {
    if (fs.existsSync(link)) fs.unlinkSync(link);
    if (fs.existsSync(file)) fs.unlinkSync(file);
    fs.rmdirSync(directory);
  }
}

async function cache() {
  var clock = now;
  var calls = [];
  var fail = false;
  var bridge = client.createBridge({now: function () { return clock; }, request: function (path, method, body, timeout) {
    calls.push({path: path, method: method, timeout: timeout});
    return fail ? Promise.reject(new Error('private upstream URL')) : Promise.resolve(path === '/api/weather' ? weather() : recovery());
  }});
  var first = await Promise.all([bridge.getState(), bridge.getState(), bridge.getState()]);
  assert.strictEqual(calls.length, 2, 'concurrent reads share one refresh');
  assert.strictEqual(first[0].recovery.rest, 82.5);
  assert.strictEqual(JSON.stringify(first[0]).indexOf('private'), -1);
  first[0].weather.temperature = 999;
  assert.strictEqual((await bridge.getState()).weather.temperature, 0, 'callers cannot mutate the cache');
  assert.strictEqual(calls.length, 2);
  clock += MINUTE;
  await bridge.getState();
  assert.strictEqual(calls.length, 3, 'recovery refreshes while weather retains valid forecast');
  assert.strictEqual(calls[2].timeout, 7000);
  fail = true;
  clock = now + 15 * MINUTE;
  var stale = await bridge.getState();
  assert.strictEqual(stale.weather.state, 'stale');
  assert.strictEqual(stale.recovery.state, 'stale');
  assert.strictEqual(stale.recovery.charge, 0, 'same-day cache retains measured zero');
  clock = Date.parse('2026-10-21T22:01:00Z');
  var next = await bridge.getState();
  assert.strictEqual(next.day, '2026-10-22');
  assert.strictEqual(next.weather.state, 'unavailable');
  assert.strictEqual(next.recovery.state, 'unavailable');
  assert.strictEqual(next.recovery.rest, null, 'previous-day health must never leak into today');

  // Separate instance: move midnight during requests and resolve both stale-day responses.
  clock = Date.parse('2026-10-21T21:59:59Z');
  var resolvers = [];
  var crossing = client.createBridge({now: function () { return clock; }, request: function () {
    return new Promise(function (resolve) { resolvers.push(resolve); });
  }});
  var waiting = crossing.getState();
  await Promise.resolve();
  clock += 2000;
  resolvers[0](weather());
  resolvers[1](recovery());
  var crossed = await waiting;
  assert.strictEqual(crossed.day, '2026-10-22');
  assert.strictEqual(crossed.recovery.state, 'unavailable');
  assert.strictEqual(crossed.weather.state, 'unavailable');
}

async function commands() {
  var clock = now;
  var calls = [];
  var release;
  var bridge = client.createBridge({now: function () { return clock; }, request: function (path, method, body, timeout) {
    calls.push({path: path, method: method, body: body, timeout: timeout});
    return new Promise(function (resolve) { release = resolve; });
  }});
  assert.strictEqual((await bridge.command('constructor')).status, 404);
  var pending = bridge.command('lights-on');
  assert.strictEqual((await bridge.command('speaker-optical')).status, 409, 'one global in-flight command');
  await Promise.resolve();
  release({message: 'private upstream acknowledgement'});
  assert.deepStrictEqual(await pending, {status: 200, body: {state: 'accepted'}});
  assert.strictEqual((await bridge.command('lights-off')).status, 429);
  clock += 1000;
  pending = bridge.command('speaker-bluetooth');
  await Promise.resolve();
  release({message: 'ok'});
  await pending;
  assert.deepStrictEqual(calls[1], {path: '/api/ir/edifier', method: 'POST', body: {command: 'bluetooth'}, timeout: 12000});
  clock += 1000;
  pending = bridge.command('speaker-optical');
  await Promise.resolve();
  release({ok: true});
  assert.strictEqual((await pending).status, 502, '2xx body without message is not an acknowledgement');
  var failures = 0;
  var failed = client.createBridge({request: function () { failures++; return Promise.reject(new Error('secret URL')); }});
  assert.deepStrictEqual(await failed.command('lights-off'), {status: 502, body: {state: 'failed', reason: 'unavailable'}});
  assert.strictEqual(failures, 1, 'failed commands must never retry automatically');
}

function localRequest(port, path, method, overrides, body) {
  return new Promise(function (resolve, reject) {
    var headers = Object.assign({Host: '127.0.0.1:39479', 'X-OpenHome-TV': '1'}, overrides);
    Object.keys(headers).forEach(function (key) { if (headers[key] === undefined) delete headers[key]; });
    var request = http.request({host: '127.0.0.1', port: port, path: path, method: method || 'GET', headers: headers}, function (response) {
      var chunks = [];
      response.on('data', function (chunk) { chunks.push(chunk); });
      response.on('end', function () { resolve({status: response.statusCode, headers: response.headers,
        body: JSON.parse(Buffer.concat(chunks).toString('utf8'))}); });
    });
    request.on('error', reject);
    request.end(body);
  });
}

async function httpBoundary() {
  var calls = [];
  var server = client.createServer({getState: function () { calls.push('read'); return Promise.resolve({day: '2026-10-21'}); },
    command: function (name) { calls.push(name); return Promise.resolve({status: 200, body: {state: 'accepted'}}); }});
  await new Promise(function (resolve) { server.listen(0, '127.0.0.1', resolve); });
  var port = server.address().port;
  try {
    assert.strictEqual((await localRequest(port, '/state')).status, 200);
    assert.strictEqual((await localRequest(port, '/state', 'GET', {'X-OpenHome-TV': undefined})).status, 403);
    assert.strictEqual((await localRequest(port, '/state', 'GET', {Host: 'attacker.invalid:39479'})).status, 403);
    assert.strictEqual((await localRequest(port, '/state', 'GET', {Origin: 'http://attacker.invalid'})).status, 403);
    assert.strictEqual((await localRequest(port, '/state', 'GET', {Origin: ''})).status, 403);
    assert.strictEqual((await localRequest(port, '/state', 'OPTIONS')).status, 404);
    assert.strictEqual((await localRequest(port, '/state?url=http://attacker.invalid')).status, 404);
    assert.strictEqual((await localRequest(port, '/command/lights-on', 'GET')).status, 404);
    assert.strictEqual((await localRequest(port, '/command/lights-on', 'POST', {'Content-Length': '1'}, 'x')).status, 413);
    assert.strictEqual((await localRequest(port, '/command/lights-on', 'POST', {'Transfer-Encoding': 'chunked'}, 'x')).status, 413);
    var accepted = await localRequest(port, '/command/lights-on', 'POST', {'Content-Length': '0'});
    assert.strictEqual(accepted.status, 200);
    assert.strictEqual(accepted.headers['access-control-allow-origin'], undefined);
    assert.strictEqual(accepted.headers['cache-control'], 'no-store');
    assert.deepStrictEqual(calls, ['read', 'lights-on'], 'rejected requests never reach the API');
  } finally { await new Promise(function (resolve) { server.close(resolve); }); }
}

async function upstreamBounds() {
  var original = https.request;
  var last;
  var mode = 'valid';
  https.request = function (options, response) {
    last = options;
    var req = new events.EventEmitter();
    req.write = function () {};
    req.destroy = function () {};
    req.end = function () {
      if (mode === 'timeout') return;
      var res = new events.EventEmitter();
      res.statusCode = mode === 'redirect' ? 302 : 200;
      res.destroy = function () {};
      response(res);
      if (mode === 'large') res.emit('data', Buffer.alloc(512 * 1024 + 1));
      else res.emit('data', Buffer.from(mode === 'invalid' ? '<html>private</html>' : '{"message":"ok"}'));
      res.emit('end');
    };
    return req;
  };
  var config = {transportHost: '100.120.132.42', serverName: 'openhome.haahr.me', apiKey: 'synthetic-key'};
  async function rejects(promise) {
    var failure;
    try { await promise; } catch (error) { failure = error; }
    assert(failure, 'request should fail');
    assert.strictEqual(failure.message, 'OpenHome request unavailable');
  }
  try {
    assert.deepStrictEqual(await client.requestApi(config, '/api/lights/on', 'POST', undefined, 100), {message: 'ok'});
    assert.strictEqual(last.hostname, '100.120.132.42');
    assert.strictEqual(last.servername, 'openhome.haahr.me');
    assert.strictEqual(last.headers.Host, 'openhome.haahr.me');
    assert.strictEqual(last.rejectUnauthorized, true);
    mode = 'large';
    await rejects(client.requestApi(config, '/api/weather', 'GET', undefined, 100));
    mode = 'redirect';
    await rejects(client.requestApi(config, '/api/weather', 'GET', undefined, 100));
    mode = 'invalid';
    await rejects(client.requestApi(config, '/api/weather', 'GET', undefined, 100));
    mode = 'timeout';
    await rejects(client.requestApi(config, '/api/weather', 'GET', undefined, 5));
  } finally { https.request = original; }
}

async function main() {
  await transforms();
  await configBoundary();
  await cache();
  await commands();
  await httpBoundary();
  await upstreamBounds();
  console.log('TV bridge: contracts, cache, midnight/DST, command acknowledgement, HTTP boundary and transport bounds passed');
}

main().catch(function (error) { console.error(error); process.exitCode = 1; });
