import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Prune shows file paths, dates and item counts in its faintest text. That is content, not
 * decoration, and it has to be readable — including by someone on a laptop screen in daylight,
 * which is where a design that looked fine on the designer's monitor usually fails.
 *
 * WCAG AA asks for 4.5:1 for ordinary text. This reads the palette out of the stylesheet and
 * checks every foreground against every surface it can sit on, in both themes, so a future
 * "just a shade lighter" cannot quietly take a whole category of information below the line.
 */

const CSS = readFileSync(join(process.cwd(), "src", "index.css"), "utf8");

const FOREGROUNDS = ["fg", "fg-muted", "fg-faint", "accent", "danger", "warn", "info"];
const BACKGROUNDS = ["bg", "bg-sidebar", "surface", "surface-2"];
const AA_NORMAL_TEXT = 4.5;

/**
 * The `:root { … }` and `.dark { … }` blocks, as name → hex.
 *
 * Anchored to the start of a line: `.dark` also appears inside Tailwind's `@custom-variant`
 * declaration, and matching that one yields an empty palette and a test that passes by
 * measuring nothing.
 */
function palette(selector: string): Record<string, string> {
  const rule = new RegExp(`^${selector.replace(".", "\\.")}\\s*\\{([^}]*)\\}`, "m");
  const body = CSS.match(rule)?.[1];
  if (body === undefined) throw new Error(`no ${selector} rule in src/index.css`);
  const out: Record<string, string> = {};
  for (const line of body.split("\n")) {
    const m = line.match(/--([a-z0-9-]+):\s*(#[0-9a-fA-F]{6})\s*;/);
    if (m?.[1] && m[2]) out[m[1]] = m[2];
  }
  return out;
}

/** Fails loudly rather than skipping a colour the stylesheet stopped defining. */
function colour(colors: Record<string, string>, name: string): string {
  const value = colors[name];
  if (!value) throw new Error(`--${name} is not defined as a hex colour`);
  return value;
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const r = channel((n >> 16) & 255);
  const g = channel((n >> 8) & 255);
  const b = channel(n & 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

describe.each([
  ["light", ":root"],
  ["dark", ".dark"],
])("the %s palette", (_theme, selector) => {
  const colors = palette(selector);

  it("defines every colour the interface asks for", () => {
    for (const name of [...FOREGROUNDS, ...BACKGROUNDS]) {
      expect(colors[name], `--${name}`).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
  });

  it("keeps every text colour readable on every surface", () => {
    const failures: string[] = [];
    for (const fg of FOREGROUNDS) {
      for (const bg of BACKGROUNDS) {
        const ratio = contrast(colour(colors, fg), colour(colors, bg));
        if (ratio < AA_NORMAL_TEXT) {
          failures.push(`${fg} on ${bg}: ${ratio.toFixed(2)}:1 (needs ${AA_NORMAL_TEXT}:1)`);
        }
      }
    }
    expect(failures).toEqual([]);
  });

  it("keeps the three levels of text visibly apart", () => {
    // Against the main background, so the hierarchy a reader relies on survives the contrast
    // fix: body text, then supporting text, then the faintest details.
    const onBg = (name: string) => contrast(colour(colors, name), colour(colors, "bg"));
    expect(onBg("fg")).toBeGreaterThan(onBg("fg-muted") * 1.5);
    expect(onBg("fg-muted")).toBeGreaterThan(onBg("fg-faint") * 1.3);
  });

  it("puts readable text on the accent button", () => {
    expect(contrast(colour(colors, "accent-fg"), colour(colors, "accent"))).toBeGreaterThanOrEqual(
      AA_NORMAL_TEXT,
    );
  });
});
