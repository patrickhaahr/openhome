'use strict';
var DAY = 86400000;
var wallClock = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Europe/Copenhagen', hour12: false, year: 'numeric', month: '2-digit', day: '2-digit',
  hour: '2-digit', minute: '2-digit', second: '2-digit'
});

function offsetAt(instant) {
  var fields = {};
  wallClock.formatToParts(new Date(instant)).forEach(function (part) { fields[part.type] = part.value; });
  var wall = Date.UTC(+fields.year, +fields.month - 1, +fields.day, +fields.hour % 24, +fields.minute, +fields.second);
  return wall - Math.floor(instant / 1000) * 1000;
}

function shiftInstant(text, days) {
  if (text === null) return null;
  var instant = Date.parse(text);
  var wall = instant + offsetAt(instant) + days * DAY;
  return new Date(wall - offsetAt(wall - offsetAt(instant))).toISOString();
}

function wholeDays(from, to) {
  return Math.round((Date.parse(to + 'T00:00:00Z') - Date.parse(from + 'T00:00:00Z')) / DAY);
}

function recoveryOn(demo, today) {
  var days = wholeDays(demo.capturedOn, today);
  var body = demo.recovery;
  return {
    day: today, time_zone: body.time_zone, derived_scores: body.derived_scores,
    noop: {freshness: body.noop.freshness, last_push_at: shiftInstant(body.noop.last_push_at, days)}
  };
}

function createRequest(options) {
  return function (path, method, body, timeoutMs) {
    var today = options.client.dayOn(options.now());
    if (method === 'GET' && path === '/api/training/days/' + today + '/recovery') {
      return Promise.resolve(recoveryOn(options.demo, today));
    }
    return options.forward(path, method, body, timeoutMs);
  };
}

module.exports = {shiftInstant: shiftInstant, wholeDays: wholeDays, recoveryOn: recoveryOn, createRequest: createRequest};

if (require.main === module) {
  var fs = require('fs');
  var client = require('/var/lib/openhome-tv/client.js');
  var config = client.loadConfig('/var/lib/openhome-tv/client.json');
  var demo = JSON.parse(fs.readFileSync('/tmp/openhome-tv-demo.json', 'utf8'));
  var request = createRequest({
    client: client, demo: demo, now: Date.now,
    forward: function (path, method, body, timeoutMs) { return client.requestApi(config, path, method, body, timeoutMs); }
  });
  client.createServer(client.createBridge({request: request})).listen(39479, '127.0.0.1');
}
