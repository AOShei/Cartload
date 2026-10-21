#!/usr/bin/env node
// Prints the WCAG contrast ratio of every text token on every surface token, in both themes,
// from the light-dark() pairs in src/app.css. Run: node scripts/contrast.mjs
// Targets: 4.5:1 for text, 3:1 for large text, icons and the faint tertiary text.
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");
const tokens = {};
for (const m of css.matchAll(/--([a-z-]+):\s*light-dark\(\s*(#[0-9a-f]{6})\s*,\s*(#[0-9a-f]{6})\s*\)/gi)) {
  tokens[m[1]] = { light: m[2], dark: m[3] };
}

const SURFACES = ["bg", "surface", "surface-raised", "surface-hover"];
const TEXT = ["text", "text-muted", "text-faint", "accent", "ok", "warn", "danger"];
const PAIRS = [
  ["on-accent", "accent", 4.5],
  ["accent-strong", "accent-soft", 4.5],
  ["ok", "ok-soft", 4.5],
  ["warn", "warn-soft", 4.5],
  ["danger", "danger-soft", 4.5],
  ["text", "accent-soft", 4.5],
  ["text", "warn-soft", 4.5],
  ["text", "danger-soft", 4.5],
];
const MIN = { text: 4.5, "text-muted": 4.5, "text-faint": 3, accent: 4.5, ok: 4.5, warn: 4.5, danger: 4.5 };

function luminance(hex) {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = c.map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}

function ratio(a, b) {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}

let failures = 0;
for (const theme of ["light", "dark"]) {
  console.log(`\n${theme}`);
  console.log("token".padEnd(14) + SURFACES.map((s) => s.padStart(16)).join(""));
  for (const t of TEXT) {
    const cells = SURFACES.map((s) => {
      const r = ratio(tokens[t][theme], tokens[s][theme]);
      const ok = r >= MIN[t];
      if (!ok) failures += 1;
      return `${r.toFixed(2)}${ok ? "  " : " !"}`.padStart(16);
    });
    console.log(t.padEnd(14) + cells.join(""));
  }
  for (const [fg, bg, min] of PAIRS) {
    const r = ratio(tokens[fg][theme], tokens[bg][theme]);
    const ok = r >= min;
    if (!ok) failures += 1;
    console.log(`${fg} on ${bg}: ${r.toFixed(2)}${ok ? "" : " !"}`);
  }
}
console.log(failures === 0 ? "\nall pairs pass" : `\n${failures} pair(s) below target`);
process.exit(failures === 0 ? 0 : 1);
