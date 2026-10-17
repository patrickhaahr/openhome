import assert from 'node:assert/strict';

const api = process.env.OPENHOME_API_URL;
const key = process.env.API_KEY;
assert.ok(api && key);
const auth = { Authorization: `Bearer ${key}` };
const health = await fetch(`${api}/api/health`, { headers: auth });
assert.equal(health.status, 200);
assert.deepEqual(await health.json(), { status: 'ok' });
assert.equal((await fetch(`${api}/api/health`)).status, 401);
console.log('API: authenticated health OK; unauthenticated access rejected.');

const origin = 'http://127.0.0.1:3000';
const response = await fetch(`${origin}/connection`);
assert.equal(response.status, 200);
const html = await response.text();
assert.ok(html.includes('Your home is connected.'));
assert.ok(!html.includes(key));
assert.ok(!html.includes(api));
const assets = [...new Set([...html.matchAll(/(?:src|href)="(\/assets\/[^"\s]+)"/g)].map(match => match[1]))];
assert.ok(assets.some(asset => asset.endsWith('.css')));
assert.ok(assets.some(asset => asset.endsWith('.js')));
for (const asset of assets) assert.equal((await fetch(`${origin}${asset}`)).status, 200);
console.log(`Dashboard: connected to API; ${assets.length} CSS/JS assets served; credentials private.`);

const mcp = 'http://openhome-mcp:8001/mcp';
assert.equal((await fetch(mcp)).status, 401);
let session;
let protocol;
async function rpc(body) {
  const headers = {
    ...auth,
    Host: process.env.MCP_SMOKE_HOST,
    Accept: 'application/json, text/event-stream',
    'Content-Type': 'application/json',
  };
  if (session) headers['Mcp-Session-Id'] = session;
  if (protocol) headers['Mcp-Protocol-Version'] = protocol;
  const response = await fetch(mcp, { method: 'POST', headers, body: JSON.stringify(body), signal: AbortSignal.timeout(15000) });
  assert.ok(response.ok, `MCP HTTP ${response.status}`);
  session = response.headers.get('mcp-session-id') ?? session;
  if (!('id' in body)) {
    await response.body?.cancel();
    return;
  }
  let data;
  if (response.headers.get('content-type')?.includes('application/json')) {
    data = await response.json();
  } else {
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let text = '';
    try {
      while (!data) {
        const chunk = await reader.read();
        assert.ok(!chunk.done, 'MCP ended without a JSON-RPC response');
        text += decoder.decode(chunk.value, { stream: true });
        const lines = text.split('\n');
        text = lines.pop();
        for (const line of lines) {
          if (!line.startsWith('data:')) continue;
          const payload = line.slice(5).trim();
          if (!payload) continue;
          const candidate = JSON.parse(payload);
          if (candidate.id === body.id) data = candidate;
        }
      }
    } finally {
      await reader.cancel();
    }
  }
  assert.ok(!data.error, `MCP returned JSON-RPC error ${data.error?.code}`);
  return data.result;
}
const initialized = await rpc({ jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'release-smoke', version: '1.0.0' } } });
assert.equal(initialized.serverInfo.version, process.env.MCP_EXPECTED_VERSION);
protocol = initialized.protocolVersion;
await rpc({ jsonrpc: '2.0', method: 'notifications/initialized' });
const listed = await rpc({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} });
assert.ok(listed.tools.some(tool => tool.name === 'wishlists' && tool.annotations?.readOnlyHint === true));
const read = await rpc({ jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'wishlists', arguments: {} } });
assert.ok(!read.isError, 'MCP read-only API call must succeed');
assert.ok(read.content.length > 0);
console.log(`MCP: version ${initialized.serverInfo.version}; ${listed.tools.length} tools; read-only API call OK; unauthenticated access rejected.`);
if (session) {
  await fetch(mcp, { method: 'DELETE', headers: { ...auth, Host: process.env.MCP_SMOKE_HOST, 'Mcp-Session-Id': session, 'Mcp-Protocol-Version': protocol }, signal: AbortSignal.timeout(5000) });
}
