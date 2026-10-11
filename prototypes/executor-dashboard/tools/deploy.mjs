import { readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

function cli(args) {
  const result = spawnSync(process.execPath, ["tools/executor.mjs", ...args], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || "Executor command failed");
  return JSON.parse(result.stdout);
}

let app;
try {
  app = JSON.parse(readFileSync(".runtime/prototype.json", "utf8")).app;
} catch (error) {
  if (error.code !== "ENOENT") throw error;
  const existing = cli(["apps", "list"]).find(
    (item) => item.name === "OpenHome dashboard prototype",
  );
  app =
    existing?.id ??
    cli(["apps", "create", "--name", "OpenHome dashboard prototype", "--files", "app"]).id;
  writeFileSync(".runtime/prototype.json", JSON.stringify({ app }));
}
const source = cli(["apps", "source", "--app", app]);
const revision = cli([
  "apps",
  "commit",
  "--app",
  app,
  "--files",
  "app",
  "--expected",
  source.revision.commit,
  "--message",
  "Update dashboard prototype",
]);
const commit = revision.commit ?? revision.revision?.commit;
if (typeof commit !== "string") throw new Error("Executor returned no source commit");
const result = cli(["apps", "deploy", "--app", app, "--commit", commit]);
console.log(JSON.stringify({ app, commit, deployment: result }, null, 2));
