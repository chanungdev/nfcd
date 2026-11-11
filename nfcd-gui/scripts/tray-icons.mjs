// Render the menu bar icons from Lucide SVGs: `node scripts/tray-icons.mjs`.
// Black on transparent, used as macOS template images (tinted for light/dark).
import { readFileSync, writeFileSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const ICONS = { running: "file-type-corner", stopped: "file-x-corner" };
const SIZE = 36; // 18pt menu bar icon at @2x

for (const [state, name] of Object.entries(ICONS)) {
  const svg = readFileSync(`node_modules/lucide-static/icons/${name}.svg`, "utf8");
  const png = new Resvg(svg, { fitTo: { mode: "width", value: SIZE } }).render().asPng();
  writeFileSync(`src-tauri/icons/tray/${state}.png`, png);
  console.log(`${state}: ${name}`);
}
