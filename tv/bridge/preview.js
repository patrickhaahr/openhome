'use strict';
// Developer screenshot fixture, run only temporarily on the TV. Never installed
// by tv-deploy; no key, provider requests, microphone or working command routes.
var client = require('/var/lib/openhome-tv/client.js');
var now = Date.now();
var day = client.dayOn(now);
function score(value) { return {value: value, unit: 'score_0_100'}; }
var fixture = {
  '/api/weather': {state: 'fresh', forecast: {
    timezone: 'Europe/Copenhagen', fetched_at: new Date(now).toISOString(), valid_until: new Date(now + 900000).toISOString(),
    current: {observed_at: new Date(now).toISOString(), temperature_c: 12, weather_code: 2, wind_ms: 3},
    days: [{date: day, high_c: 15, low_c: 8, weather_code: 2, precipitation_mm: 0, wind_ms: 3}]
  }}
};
fixture['/api/training/days/' + day + '/recovery'] = {
  day: day, time_zone: 'Europe/Copenhagen',
  derived_scores: {recovery: score(76), rest: score(88), strain: score(42)},
  noop: {freshness: 'confirmed', last_push_at: new Date(now).toISOString()}
};
var bridge = client.createBridge({request: function (path, method) {
  return method === 'GET' && fixture[path] ? Promise.resolve(fixture[path]) : Promise.reject(new Error('Preview controls disabled'));
}});
client.createServer(bridge).listen(39479, '127.0.0.1');
