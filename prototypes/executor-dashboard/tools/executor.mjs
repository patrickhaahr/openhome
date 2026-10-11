import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const dataDir = resolve(".runtime/data");
const keys = JSON.parse(readFileSync(resolve(dataDir, "keys.json"), "utf8"));
if (typeof keys.apiKey !== "string" || keys.apiKey.length === 0) {
  throw new Error("The prototype Executor has no local API key. Start just serve first.");
}
const result = spawnSync(resolve(".runtime/node_modules/.bin/executor"), process.argv.slice(2), {
  env: {
    ...process.env,
    EXECUTOR_DATA_DIR: dataDir,
    ...(process.argv[2] === "pair"
      ? { EXECUTOR_KEY_STORAGE: "file" }
      : { EXECUTOR_API_KEY: keys.apiKey }),
  },
  stdio: "inherit",
});
process.exit(result.status ?? 1);
