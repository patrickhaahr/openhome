'use strict';

var fs = require('fs');
var http = require('http');
var https = require('https');
var net = require('net');
var lifecycles = require('./lifecycles');

var PORT = 39479;
var MAX_BYTES = 512 * 1024;
var MINUTE = 60 * 1000;
var MAX_WEATHER_AGE = 6 * 60 * MINUTE;
var COMMANDS = {
  'lights-on': {path: '/api/lights/on'},
  'lights-off': {path: '/api/lights/off'},
  'speaker-bluetooth': {path: '/api/ir/edifier', body: {command: 'bluetooth'}},
  'speaker-optical': {path: '/api/ir/edifier', body: {command: 'optical'}}
};
var has = function (object, name) { return Object.prototype.hasOwnProperty.call(object, name); };
var copenhagen = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Europe/Copenhagen', year: 'numeric', month: '2-digit', day: '2-digit'
});

function dayOn(now) {
  var fields = {};
  copenhagen.formatToParts(new Date(now)).forEach(function (part) { fields[part.type] = part.value; });
  return fields.year + '-' + fields.month + '-' + fields.day;
}

function timestamp(value) {
  return typeof value === 'string' && /T.*(?:Z|[+-]\d\d:\d\d)$/.test(value) && isFinite(Date.parse(value));
}

function optionalNumber(value, minimum, maximum) {
  return value === null || (typeof value === 'number' && isFinite(value) && value >= minimum && value <= maximum);
}

function unavailableWeather() {
  return {state: 'unavailable', temperature: null, code: null, high: null, low: null,
    rain: null, wind: null, fetchedAt: null, validUntil: null};
}

function unavailableRecovery() {
  return {state: 'unavailable', charge: null, rest: null, effort: null, freshness: 'unknown', updatedAt: null};
}

function weatherFrom(body, day, now) {
  if (!body || (body.state !== 'fresh' && body.state !== 'stale')) return null;
  var forecast = body.forecast;
  if (!forecast || forecast.timezone !== 'Europe/Copenhagen' || !timestamp(forecast.fetched_at) ||
      !timestamp(forecast.valid_until) || !Array.isArray(forecast.days)) return null;
  var fetched = Date.parse(forecast.fetched_at);
  var valid = Date.parse(forecast.valid_until);
  if (fetched > now + MINUTE || valid <= fetched || valid - fetched > MAX_WEATHER_AGE ||
      now - fetched >= MAX_WEATHER_AGE) return null;
  var today = forecast.days.filter(function (entry) { return entry && entry.date === day; })[0];
  if (!today) return null;
  var current = forecast.current;
  if (current !== null && (!current || !timestamp(current.observed_at))) return null;
  var temperature = current === null ? null : current.temperature_c;
  var code = current === null ? today.weather_code : current.weather_code;
  var wind = current === null ? today.wind_ms : current.wind_ms;
  if (!optionalNumber(temperature, -100, 100) || !optionalNumber(code, 0, 255) ||
      (code !== null && Math.floor(code) !== code) || !optionalNumber(today.high_c, -100, 100) ||
      !optionalNumber(today.low_c, -100, 100) || !optionalNumber(today.precipitation_mm, 0, 10000) ||
      !optionalNumber(wind, 0, 200)) return null;
  return {state: body.state === 'fresh' && valid > now ? 'fresh' : 'stale',
    temperature: temperature, code: code, high: today.high_c, low: today.low_c,
    rain: today.precipitation_mm, wind: wind,
    fetchedAt: forecast.fetched_at, validUntil: forecast.valid_until};
}

function recoveryFrom(body, day) {
  if (!body || body.day !== day || body.time_zone !== 'Europe/Copenhagen' || !body.derived_scores || !body.noop) return null;
  var scores = body.derived_scores;
  var names = ['recovery', 'rest', 'strain'];
  if (!names.every(function (name) {
    return scores[name] && scores[name].unit === 'score_0_100' && optionalNumber(scores[name].value, 0, 100);
  })) return null;
  if (['confirmed', 'partial', 'unconfirmed', 'unknown'].indexOf(body.noop.freshness) === -1 ||
      !(body.noop.last_push_at === null || timestamp(body.noop.last_push_at))) return null;
  return {state: 'fresh', charge: scores.recovery.value, rest: scores.rest.value, effort: scores.strain.value,
    freshness: body.noop.freshness, updatedAt: body.noop.last_push_at};
}

function loadConfig(path) {
  var stat = fs.lstatSync(path);
  if (!stat.isFile() || (stat.mode & 0o777) !== 0o600 || stat.uid !== process.getuid()) {
    throw new Error('TV client configuration must be an owner-only regular file');
  }
  var config = JSON.parse(fs.readFileSync(path, 'utf8'));
  if (!config || typeof config.apiKey !== 'string' || !config.apiKey || /\s/.test(config.apiKey) ||
      config.apiKey.length > 8192 || typeof config.transportHost !== 'string' || !net.isIP(config.transportHost) ||
      config.serverName !== 'openhome.haahr.me') throw new Error('Invalid TV client configuration');
  return config;
}

function requestApi(config, path, method, body, timeoutMs) {
  return new Promise(function (resolve, reject) {
    var finished = false;
    var req;
    var timer;
    function complete(error, value) {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      if (error) { if (req) req.destroy(); reject(new Error('OpenHome request unavailable')); }
      else resolve(value);
    }
    var encoded = body === undefined ? null : JSON.stringify(body);
    var headers = {Host: config.serverName, Authorization: 'Bearer ' + config.apiKey, Accept: 'application/json'};
    if (encoded !== null) {
      headers['Content-Type'] = 'application/json';
      headers['Content-Length'] = Buffer.byteLength(encoded);
    }
    req = https.request({hostname: config.transportHost, port: 443, servername: config.serverName,
      method: method, path: path, headers: headers, rejectUnauthorized: true, agent: false}, function (res) {
      var bytes = 0;
      var chunks = [];
      res.on('error', function () { complete(true); });
      res.on('aborted', function () { complete(true); });
      res.on('data', function (chunk) {
        bytes += chunk.length;
        if (bytes > MAX_BYTES) { complete(true); res.destroy(); }
        else chunks.push(chunk);
      });
      res.on('end', function () {
        if (finished) return;
        if (res.statusCode < 200 || res.statusCode >= 300) return complete(true);
        try { complete(false, JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
        catch (_) { complete(true); }
      });
    });
    req.on('error', function () { complete(true); });
    timer = setTimeout(function () { complete(true); }, timeoutMs);
    if (encoded !== null) req.write(encoded);
    req.end();
  });
}

function createBridge(options) {
  var now = options.now || Date.now;
  var request = options.request;
  var today = dayOn(now());
  var observations = {weather: lifecycles.unseen(today), recovery: lifecycles.unseen(today)};
  var refresh = {state: 'idle'};
  var admission = {state: 'idle', nextAt: 0};

  function observe(name, event) { observations[name] = lifecycles.observe(observations[name], event); }

  function rollDay() {
    var at = now();
    var tick = {type: 'tick', at: at, day: dayOn(at)};
    observe('weather', tick);
    observe('recovery', tick);
    return tick;
  }

  function weatherEvent(body, day) {
    var reading = weatherFrom(body, day, now());
    if (!reading) return {type: 'missed', day: day};
    var staleAt = Date.parse(reading.validUntil);
    return {type: 'received', day: day, reading: reading, staleAt: staleAt,
      dueAt: reading.state === 'fresh' ? staleAt : now() + lifecycles.RETRY_AFTER,
      discardAt: Date.parse(reading.fetchedAt) + MAX_WEATHER_AGE};
  }

  function recoveryEvent(body, day) {
    var reading = recoveryFrom(body, day);
    return reading ? {type: 'received', day: day, reading: reading} : {type: 'missed', day: day};
  }

  function fetchInto(flight, name, path, tick, eventFrom) {
    observe(name, {type: 'requested', at: tick.at});
    function settle(event) { if (lifecycles.isCurrent(refresh, flight)) observe(name, event); }
    return Promise.resolve().then(function () { return request(path, 'GET', undefined, 7000); }).then(function (body) {
      rollDay();
      settle(eventFrom(body, tick.day));
    }, function () {
      rollDay();
      settle({type: 'missed', day: tick.day});
    });
  }

  function allSettled(tasks) {
    var failures = [];
    return Promise.all(tasks.map(function (task) {
      return task.catch(function (error) { failures.push(error); });
    })).then(function () { if (failures.length) throw failures[0]; });
  }

  function shown(observation, unavailable) {
    return observation.state === 'unavailable' ? unavailable : Object.assign({}, observation.reading, {state: observation.state});
  }

  function snapshot() {
    return {day: rollDay().day, weather: shown(observations.weather, unavailableWeather()),
      recovery: shown(observations.recovery, unavailableRecovery())};
  }

  function getState() {
    var tick = rollDay();
    if (refresh.state === 'refreshing') return refresh.flight.result.then(getState);
    var flight = {};
    var tasks = [];
    refresh = lifecycles.refresh(refresh, {type: 'start', flight: flight});
    if (tick.at >= observations.weather.dueAt) {
      tasks.push(fetchInto(flight, 'weather', '/api/weather', tick, weatherEvent));
    }
    if (tick.at >= observations.recovery.dueAt) {
      tasks.push(fetchInto(flight, 'recovery', '/api/training/days/' + tick.day + '/recovery', tick, recoveryEvent));
    }
    function settle() { refresh = lifecycles.refresh(refresh, {type: 'settle', flight: flight}); }
    flight.result = allSettled(tasks).then(function () { settle(); return snapshot(); },
      function (error) { settle(); throw error; });
    return flight.result;
  }

  function command(name) {
    if (!has(COMMANDS, name)) return Promise.resolve({status: 404, body: {state: 'failed', reason: 'unknown-command'}});
    var flight = {};
    var admitted = lifecycles.admit(admission, {type: 'send', at: now(), flight: flight});
    if (admitted === admission) {
      return Promise.resolve(admission.state === 'sending' ?
        {status: 409, body: {state: 'failed', reason: 'busy'}} :
        {status: 429, body: {state: 'failed', reason: 'rate-limited'}});
    }
    admission = admitted;
    function settle() { admission = lifecycles.admit(admission, {type: 'settle', flight: flight}); }
    var action = COMMANDS[name];
    return Promise.resolve().then(function () { return request(action.path, 'POST', action.body, 12000); }).then(function (body) {
      settle();
      return body && typeof body.message === 'string' ? {status: 200, body: {state: 'accepted'}} :
        {status: 502, body: {state: 'failed', reason: 'unavailable'}};
    }, function () {
      settle();
      return {status: 502, body: {state: 'failed', reason: 'unavailable'}};
    });
  }
  return {getState: getState, command: command};
}

function createServer(bridge) {
  var server = http.createServer(function (req, res) {
    function reply(status, body, close) {
      res.writeHead(status, {'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store',
        'X-Content-Type-Options': 'nosniff', Connection: close ? 'close' : 'keep-alive'});
      res.end(JSON.stringify(body));
      if (close) res.on('finish', function () { req.destroy(); });
    }
    if (req.headers.host !== '127.0.0.1:' + PORT || req.headers['x-openhome-tv'] !== '1' || has(req.headers, 'origin')) {
      return reply(403, {state: 'failed', reason: 'forbidden'}, true);
    }
    if (has(req.headers, 'transfer-encoding') || (has(req.headers, 'content-length') && req.headers['content-length'] !== '0')) {
      return reply(413, {state: 'failed', reason: 'body-not-allowed'}, true);
    }
    req.resume();
    if (req.method === 'GET' && req.url === '/state') {
      return bridge.getState().then(function (state) { reply(200, state); }, function () {
        reply(503, {state: 'failed', reason: 'unavailable'});
      });
    }
    var match = /^\/command\/([a-z-]+)$/.exec(req.url);
    if (req.method === 'POST' && match) {
      return bridge.command(match[1]).then(function (result) { reply(result.status, result.body); });
    }
    reply(404, {state: 'failed', reason: 'not-found'});
  });
  server.timeout = 15000;
  return server;
}

module.exports = {dayOn: dayOn, weatherFrom: weatherFrom, recoveryFrom: recoveryFrom,
  createBridge: createBridge, createServer: createServer, requestApi: requestApi, loadConfig: loadConfig};

if (require.main === module) {
  try {
    var config = loadConfig('/var/lib/openhome-tv/client.json');
    var bridge = createBridge({request: function (path, method, body, timeoutMs) {
      return requestApi(config, path, method, body, timeoutMs);
    }});
    var server = createServer(bridge);
    server.on('error', function () { console.error('OpenHome TV bridge could not start'); process.exit(1); });
    server.listen(PORT, '127.0.0.1', function () { console.log('OpenHome TV bridge ready'); });
    process.on('SIGTERM', function () {
      setTimeout(function () { process.exit(0); }, 1500).unref();
      server.close(function () { process.exit(0); });
    });
  } catch (_) {
    console.error('OpenHome TV bridge configuration unavailable');
    process.exit(1);
  }
}
