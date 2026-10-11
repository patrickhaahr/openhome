import { spawnSync } from "node:child_process";
const paired = spawnSync(process.execPath, ["tools/executor.mjs", "pair"], { encoding: "utf8" });
if (paired.status !== 0)
  throw new Error(paired.stderr || "Could not create a local browser pairing link");
const url = paired.stdout.match(
  /http:\/\/(?:127\.0\.0\.1|localhost):4312\/[^\s]*#pair=[a-f0-9]+/,
)?.[0];
if (url === undefined) throw new Error("Executor did not return a pairing URL");
const opened = spawnSync("just", ["browser", "open", url], { encoding: "utf8" });
if (opened.status !== 0) throw new Error("Local browser pairing navigation failed");
console.log("Opened the one-use local pairing link in the prototype browser.");
