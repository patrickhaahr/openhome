import { parseArgs } from "node:util";

const { values, positionals } = parseArgs({
  args: process.argv.slice(2),
  allowPositionals: true,
  options: { directory: { type: "string", default: "http://127.0.0.1:9222/json" } },
});
if (positionals.length !== 2) {
  console.error("Usage: bun inspect.mjs commands.json exact-page-url [--directory http://127.0.0.1:9222/json]");
  process.exit(1);
}
const commands = JSON.parse(await Bun.file(positionals[0]).text());
if (!Array.isArray(commands)) throw new Error("Commands must be a JSON array");
const response = await fetch(values.directory, { signal: AbortSignal.timeout(5000) });
if (!response.ok) throw new Error(`Page directory returned HTTP ${response.status}`);
const pages = await response.json();
const matches = pages.filter(page => page.url === positionals[1]);
if (matches.length !== 1 || !matches[0].webSocketDebuggerUrl)
  throw new Error("Expected one inspectable tab matching the exact dashboard URL");

const socket = new WebSocket(matches[0].webSocketDebuggerUrl);
let targetId;
let outerId = 1000;
let innerId = 0;
let started = false;
const pending = new Map();
const timeout = setTimeout(() => fail(new Error("Inspector timed out")), 20000);

function fail(error) {
  console.error(error);
  clearTimeout(timeout);
  socket.close();
  process.exitCode = 1;
}

function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++innerId;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({
      id: ++outerId,
      method: "Target.sendMessageToTarget",
      params: { targetId, message: JSON.stringify({ id, method, params }) },
    }));
  });
}

socket.onerror = () => fail(new Error("Inspector WebSocket failed"));
socket.onclose = () => {
  if (pending.size) fail(new Error("Inspector disconnected with commands pending"));
};
socket.onmessage = async event => {
  try {
    const message = JSON.parse(event.data);
    if (message.error) throw new Error(JSON.stringify(message.error));
    if (message.method === "Target.didCommitProvisionalTarget")
      targetId = message.params.newTargetId;
    if (message.method === "Target.targetCreated" && !started) {
      started = true;
      targetId = message.params.targetInfo.targetId;
      for (const command of commands) {
        if (command.delay) { await Bun.sleep(command.delay); continue; }
        const result = await send(command.method, command.params);
        if (result?.wasThrown)
          throw new Error(result.result?.description ?? "Evaluation threw");
        if (command.saveImage) {
          if (!result?.dataURL?.startsWith("data:image/png;base64,"))
            throw new Error("Snapshot did not return PNG data");
          await Bun.write(command.saveImage, Buffer.from(result.dataURL.split(",")[1], "base64"));
          console.log(JSON.stringify({ savedImage: command.saveImage }));
        } else if (result?.dataURL) {
          console.log(JSON.stringify({ method: command.method, snapshot: "Use saveImage to save PNG data" }));
        } else console.log(JSON.stringify({ method: command.method, result }));
      }
      clearTimeout(timeout);
      socket.close();
    } else if (message.method === "Target.dispatchMessageFromTarget") {
      const inner = JSON.parse(message.params.message);
      if (inner.id && pending.has(inner.id)) {
        const { resolve, reject } = pending.get(inner.id);
        pending.delete(inner.id);
        inner.error ? reject(new Error(JSON.stringify(inner.error))) : resolve(inner.result);
      } else if (inner.method === "Console.messageAdded") {
        const entry = inner.params.message;
        if (entry.level === "error" || entry.level === "warning")
          console.log(JSON.stringify({ console: { level: entry.level, text: entry.text } }));
      }
    }
  } catch (error) { fail(error); }
};
