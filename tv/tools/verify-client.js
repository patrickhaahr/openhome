'use strict';
// Read-only checks on the TV's own Node 8 runtime. No light or IR commands.
var assert = require('assert');
var fs = require('fs');
var http = require('http');
var config = fs.statSync('/var/lib/openhome-tv/client.json');
assert.strictEqual(config.mode & 0o777, 0o600);
assert.strictEqual(config.uid, 0);
function read(path, extra) {
  return new Promise(function (resolve, reject) {
    var headers = {'X-OpenHome-TV': '1'};
    Object.keys(extra || {}).forEach(function (key) { headers[key] = extra[key]; });
    var req = http.get({hostname: '127.0.0.1', port: 39479, path: path, headers: headers}, function (res) {
      var body = ''; res.on('data', function (chunk) { body += chunk; });
      res.on('end', function () { resolve({code: res.statusCode, data: JSON.parse(body)}); });
    });
    req.on('error', reject);
    req.setTimeout(10000, function () { req.destroy(new Error('TV client did not answer')); });
  });
}
Promise.all([read('/state'), read('/state', {Origin: 'https://example.invalid'}), read('/api/health')]).then(function (results) {
  assert.strictEqual(results[0].code, 200);
  var state = results[0].data;
  assert.strictEqual(state.day, require('/var/lib/openhome-tv/client.js').dayOn(Date.now()));
  assert.strictEqual(state.weather.state, 'fresh', 'Live weather must be fresh for this acceptance check');
  assert.strictEqual(state.recovery.state, 'fresh', 'Live recovery read must succeed');
  ['charge', 'rest', 'effort'].forEach(function (name) {
    assert(state.recovery[name] === null || typeof state.recovery[name] === 'number');
  });
  assert.strictEqual(results[1].code, 403, 'Browser-origin requests must be rejected');
  assert.strictEqual(results[2].code, 404, 'No generic API proxy');
  var listeners = fs.readFileSync('/proc/net/tcp', 'utf8').split('\n').filter(function (line) {
    var columns = line.trim().split(/\s+/);
    return columns[1] && columns[1].split(':')[1] === (39479).toString(16).toUpperCase() && columns[3] === '0A';
  });
  assert.strictEqual(listeners.length, 1);
  assert.strictEqual(listeners[0].trim().split(/\s+/)[1].split(':')[0], '0100007F');
  console.log('PASS: Node8, authenticated API reads, fresh weather/recovery, root-only credential, loopback listener and HTTP boundary');
}).catch(function (error) { console.error(error.message); process.exit(1); });
