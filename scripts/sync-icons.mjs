// Regenerates assets/icons from Hugeicons (MIT). Usage:
//   npm i --prefix <scratch> @iconify-json/hugeicons
//   node scripts/sync-icons.mjs <scratch>/node_modules/@iconify-json/hugeicons/icons.json
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const out = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "assets", "icons");
const set = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));

const hugeicons = {
  area: "full-screen",
  fullscreen: "computer",
  window: "app-window",
  timer: "timer-01",
  ocr: "text-font",
  history: "history",
  pin: "pin",
  copy: "copy-01",
  save: "download-01",
  edit: "pencil-edit-02",
  close: "cancel-01",
  cursor: "cursor-01",
  crop: "crop",
  arrow: "arrow-up-right-01",
  rect: "square",
  ellipse: "circle",
  pencil: "pen-01",
  highlighter: "highlighter",
  text: "text",
  counter: "one-circle",
  blur: "droplet",
  pixelate: "grid3x3",
  spotlight: "spotlight",
  undo: "undo-02",
  redo: "redo-02",
  "rotate-left": "rotate-ccw-square",
  "rotate-right": "rotate-cw-square",
  "flip-h": "flip-horizontal",
  "flip-v": "flip-vertical",
  resize: "expand",
  trash: "delete-02",
  background: "wallpaper",
  drag: "drag-drop",
  settings: "settings-01",
  check: "tick-02",
  lock: "square-lock-01",
  unlock: "square-unlock-01",
  folder: "folder-01",
  restore: "square-arrow-reload-01",
  keyboard: "keyboard",
  image: "image-01",
  sparkles: "sparkles",
  eye: "view",
  "eye-off": "view-off-slash",
  minus: "minus-sign",
  maximize: "square",
  "zoom-in": "zoom-in",
  "zoom-out": "zoom-out",
  "chevron-down": "arrow-down-01",
  more: "more-horizontal",
  picker: "color-picker",
  info: "information-circle",
  sound: "volume-high",
  camera: "camera-01",
  "external-link": "external-link",
};

const stroke = 'fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"';
const custom = {
  "rect-filled": `<rect x="3" y="3" width="18" height="18" rx="5" fill="currentColor"/>`,
  line: `<path ${stroke} d="M5 19 19 5"/>`,
};

const wrap = (body) => `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">${body}</svg>\n`;

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
for (const [name, source] of Object.entries(hugeicons)) {
  const icon = set.icons[source] ?? set.icons[set.aliases?.[source]?.parent];
  if (!icon) throw new Error(`Hugeicons has no "${source}" (wanted for ${name})`);
  fs.writeFileSync(path.join(out, `${name}.svg`), wrap(icon.body));
}
for (const [name, body] of Object.entries(custom)) fs.writeFileSync(path.join(out, `${name}.svg`), wrap(body));
console.log(`${Object.keys(hugeicons).length + Object.keys(custom).length} icons -> ${out}`);
