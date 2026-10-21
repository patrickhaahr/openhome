// Export the dashboard's actual Phosphor glyphs for the TV's PNG-only Qt build.
import { createRequire } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const requireDashboard = createRequire(new URL("../dashboard/package.json", import.meta.url));
const { createElement } = requireDashboard("react");
const { renderToStaticMarkup } = requireDashboard("react-dom/server");
const output = new URL("./private/icons/", import.meta.url);
mkdirSync(output, { recursive: true });
const glyphs = {
  bulb: ["LightbulbFilament", "fill"], "bulb-off": ["Lightbulb", "regular"],
  speaker: ["SpeakerHifi", "regular"], eye: ["Eye", "regular"],
  bluetooth: ["Bluetooth", "regular"], optical: ["Waveform", "regular"],
  charge: ["Lightning", "fill"], rest: ["Moon", "fill"], effort: ["Heartbeat", "fill"],
  sun: ["Sun", "regular"], cloud: ["Cloud", "regular"], "cloud-sun": ["CloudSun", "regular"],
  fog: ["CloudFog", "regular"], rain: ["CloudRain", "regular"], snow: ["CloudSnow", "regular"],
  storm: ["CloudLightning", "regular"], unknown: ["Question", "regular"],
  down: ["CaretDown", "regular"], up: ["CaretUp", "regular"], grid: ["SquaresFour", "regular"],
  close: ["X", "regular"], stale: ["ClockCounterClockwise", "regular"],
  warning: ["WarningCircle", "fill"], play: ["Play", "fill"], pause: ["Pause", "fill"],
};
const colours = ["ecece6", "000000", "3ddc84", "ffcc33", "ff6b5e", "5b9dff", "9a9ea6", "ff8a7a"];
for (const [kind, [name, weight]] of Object.entries(glyphs)) {
  const module = await import(new URL(`../dashboard/node_modules/@phosphor-icons/react/dist/csr/${name}.es.js`, import.meta.url));
  for (const ink of colours) {
    const svg = renderToStaticMarkup(createElement(module[`${name}Icon`], { size: 256, weight, color: `#${ink}` }));
    const stem = new URL(`${kind}-${ink}`, output);
    writeFileSync(`${fileURLToPath(stem)}.svg`, svg);
    const rendered = spawnSync("rsvg-convert", ["--width", "256", "--height", "256", `${fileURLToPath(stem)}.svg`, "-o", `${fileURLToPath(stem)}.png`], { stdio: "inherit" });
    if (rendered.status !== 0) throw new Error(`Could not render ${kind}`);
  }
}
console.log("Dashboard Phosphor icons exported for the TV.");

// The agent's authored vector layers stay cached while Qt rotates the rotor.
function arc(radius, width, colour, start, end) {
  const point = (angle) => [150 + radius * Math.cos(angle * Math.PI / 180), 150 + radius * Math.sin(angle * Math.PI / 180)];
  const [x1, y1] = point(start), [x2, y2] = point(end);
  return `<path d="M${x1},${y1} A${radius},${radius} 0 ${end - start > 180 ? 1 : 0} 1 ${x2},${y2}" fill="none" stroke="${colour}" stroke-width="${width}"/>`;
}
const circle = (r, width, colour) => `<circle cx="150" cy="150" r="${r}" fill="none" stroke="${colour}" stroke-width="${width}"/>`;
let frame = circle(130, 1, "#075874") + circle(107, 1, "#12627a") + circle(78, 1.5, "#0875a0") + circle(69, 0.8, "#0b536a");
for (let i = 0; i < 4; i++) frame += arc(143, 0.7, "#12627a", 4 + i * 90, 88 + i * 90);
for (let i = 0; i < 72; i++) {
  const a = i * Math.PI / 36, outer = i % 6 === 0 ? 139 : 136;
  frame += `<path d="M${150 + 132 * Math.cos(a)},${150 + 132 * Math.sin(a)} L${150 + outer * Math.cos(a)},${150 + outer * Math.sin(a)}" stroke="${i % 6 === 0 ? "#00dbe8" : "#145773"}" stroke-width="${i % 6 === 0 ? 1.1 : 0.55}"/>`;
}
for (const p of [[2,39,36,39,49,60],[295,251,268,251,255,233],[5,228,23,228,32,213],[292,91,275,91,266,107]]) {
  frame += `<path d="M${p[0]},${p[1]} L${p[2]},${p[3]} L${p[4]},${p[5]}" fill="none" stroke="#0c7d9a" stroke-width="0.8"/><rect x="${p[0]-1.5}" y="${p[1]-1.5}" width="3" height="3" fill="#00dbe8"/>`;
}
const rotor = [
  [119,5,"#00bace",-83,-18], [119,5,"#07599c",-11,37], [119,5,"#00bace",44,92],
  [119,5,"#07599c",99,167], [119,5,"#07599c",174,214], [119,5,"#00bace",221,267],
  [98,9,"#0863a3",-78,-25], [98,9,"#00dbe8",-18,20], [98,9,"#0863a3",28,83],
  [98,9,"#00dbe8",119,151], [98,9,"#0863a3",160,203], [98,9,"#0863a3",239,275],
  [88,1,"#00a9c1",-80,59], [88,1,"#0b68a0",80,260],
].map((args) => arc(...args)).join("");
for (const [name, drawing] of [["agent-frame", frame], ["agent-rotor", rotor]]) {
  const path = fileURLToPath(new URL(name, output));
  writeFileSync(`${path}.svg`, `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 300">${drawing}</svg>`);
  const result = spawnSync("rsvg-convert", ["--width", "1024", "--height", "1024", `${path}.svg`, "-o", `${path}.png`], {stdio:"inherit"});
  if (result.status !== 0) throw new Error(`Could not render ${name}`);
}
