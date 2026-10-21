'use strict';

// Node 8.12 ships on this TV. Keep this client dependency-free and compatible with it.
var fs = require('fs');
var http = require('http');
var https = require('https');
var net = require('net');

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

// Host/SNI retain the certificate hostname while the TCP connection uses the private Pi address.
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
  var cachedDay = dayOn(now());
  var weather = unavailableWeather();
  var recovery = unavailableRecovery();
  var weatherAt = 0;
  var recoveryAt = 0;
  var refreshing = null;
  var commandPending = false;
  var nextCommand = 0;

  function rollDay() {
    var day = dayOn(now());
    if (day !== cachedDay) {
      cachedDay = day;
      weather = unavailableWeather();
      recovery = unavailableRecovery();
      weatherAt = recoveryAt = 0;
    }
    if (weather.state === 'fresh' && Date.parse(weather.validUntil) <= now()) weather.state = 'stale';
    if (weather.fetchedAt && now() - Date.parse(weather.fetchedAt) >= MAX_WEATHER_AGE) {
      weather = unavailableWeather();
      weatherAt = 0;
    }
    return day;
  }

  function getState() {
    var day = rollDay();
    if (refreshing) return refreshing.then(getState);
    var tasks = [];
    if (now() >= weatherAt) {
      weatherAt = now() + MINUTE;
      tasks.push(Promise.resolve().then(function () { return request('/api/weather', 'GET', undefined, 7000); }).then(function (body) {
        if (rollDay() !== day) return;
        var next = weatherFrom(body, day, now());
        if (next) {
          weather = next;
          weatherAt = next.state === 'fresh' ? Date.parse(next.validUntil) : now() + MINUTE;
        } else if (weather.state !== 'unavailable') weather.state = 'stale';
      }, function () { if (rollDay() === day && weather.state !== 'unavailable') weather.state = 'stale'; }));
    }
    if (now() >= recoveryAt) {
      recoveryAt = now() + MINUTE;
      tasks.push(Promise.resolve().then(function () { return request('/api/training/days/' + day + '/recovery', 'GET', undefined, 7000); }).then(function (body) {
        if (rollDay() !== day) return;
        var next = recoveryFrom(body, day);
        if (next) recovery = next;
        else if (recovery.state !== 'unavailable') recovery.state = 'stale';
      }, function () { if (rollDay() === day && recovery.state !== 'unavailable') recovery.state = 'stale'; }));
    }
    refreshing = Promise.all(tasks).then(function () {
      refreshing = null;
      rollDay();
      // Return only the display contract, never the upstream response objects.
      return JSON.parse(JSON.stringify({day: cachedDay, weather: weather, recovery: recovery}));
    });
    return refreshing;
  }

  function command(name) {
    if (!has(COMMANDS, name)) return Promise.resolve({status: 404, body: {state: 'failed', reason: 'unknown-command'}});
    if (commandPending) return Promise.resolve({status: 409, body: {state: 'failed', reason: 'busy'}});
    if (now() < nextCommand) return Promise.resolve({status: 429, body: {state: 'failed', reason: 'rate-limited'}});
    commandPending = true;
    nextCommand = now() + 1000;
    var action = COMMANDS[name];
    return Promise.resolve().then(function () { return request(action.path, 'POST', action.body, 12000); }).then(function (body) {
      commandPending = false;
      return body && typeof body.message === 'string' ? {status: 200, body: {state: 'accepted'}} :
        {status: 502, body: {state: 'failed', reason: 'unavailable'}};
    }, function () {
      commandPending = false;
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
      // A redeploy must not leave an old keep-alive listener holding the port.
      setTimeout(function () { process.exit(0); }, 1500).unref();
      server.close(function () { process.exit(0); });
    });
  } catch (_) {
    console.error('OpenHome TV bridge configuration unavailable');
    process.exit(1);
  }
}
