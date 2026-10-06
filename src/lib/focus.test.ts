import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * A text field drawn as a bordered box around a bare `<input>` gives a keyboard user no sign of
 * where they are unless the box carries the focus ring: the input's own outline is removed
 * precisely because the box is the field. Three of them showed nothing at all when focused.
 *
 * jsdom does not load the stylesheet, so this cannot ask what is drawn. It reads the source, as
 * `no-network.test.ts` does, and fails on the line someone wrote.
 */
const SRC = join(process.cwd(), "src");

/** Fields that are deliberately not boxed: the command palette is one input in a dialog of its own. */
const EXEMPT = new Set([join("features", "palette", "CommandPalette.tsx")]);

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const full = join(dir, e.name);
    if (e.isDirectory()) return sources(full);
    return /\.tsx$/.test(e.name) && !/\.test\.tsx$/.test(e.name) ? [full] : [];
  });
}

describe("text fields", () => {
  it("every input that removes its outline sits in a box that takes the focus ring", () => {
    const offenders: string[] = [];
    for (const file of sources(SRC)) {
      const rel = relative(SRC, file);
      if (EXEMPT.has(rel)) continue;
      const lines = readFileSync(file, "utf8").split("\n");
      lines.forEach((line, i) => {
        if (!/outline-none/.test(line)) return;
        // The element being styled, and the ten lines above it, which hold its wrapper.
        const window = lines.slice(Math.max(0, i - 10), i + 1).join("\n");
        if (!/<input|<textarea/.test(window)) return; // a dialog panel, not a field
        if (!/["' ]field(-inset)?[" ]/.test(window)) {
          offenders.push(`${rel}:${i + 1}`);
        }
      });
    }
    expect(
      offenders,
      'add the "field" class to the box around the input, or "field-inset" where an outer ring would be clipped',
    ).toEqual([]);
  });
});
