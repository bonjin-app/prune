import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Prune does not talk to a network, and the interface is the easiest place for that to stop
 * being true: one analytics snippet, one "check for updates" call, one font loaded from a CDN,
 * and the promise on the front page is quietly false. `scripts/check-no-network.sh` does the
 * same job for the Rust side.
 *
 * This reads the source as text rather than inspecting the bundle, because the point is to fail
 * in review, on the line someone wrote, with the reason attached.
 */

// Vitest runs from the repository root.
const ROOT = process.cwd();
const SRC = join(ROOT, "src");
const THIS_FILE = join("lib", "no-network.test.ts");

/** Anything in here can reach off the machine. */
const FORBIDDEN: { pattern: RegExp; what: string }[] = [
  { pattern: /\bfetch\s*\(/, what: "fetch()" },
  { pattern: /\bXMLHttpRequest\b/, what: "XMLHttpRequest" },
  { pattern: /\bnew\s+WebSocket\b/, what: "WebSocket" },
  { pattern: /\bnew\s+EventSource\b/, what: "EventSource" },
  { pattern: /\bnavigator\s*\.\s*sendBeacon\b/, what: "navigator.sendBeacon" },
  { pattern: /\bimport\s*\(\s*["'`]https?:/, what: "a remote dynamic import" },
  { pattern: /\bfrom\s+["'`]https?:/, what: "a remote import" },
  { pattern: /<script[^>]+src=["'`]https?:/i, what: "a remote script tag" },
];

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...sourceFiles(full));
    else if (/\.(ts|tsx|js|jsx|html|css)$/.test(entry.name)) out.push(full);
  }
  return out;
}

describe("the interface", () => {
  it("never reaches off the machine", () => {
    const offences: string[] = [];
    for (const file of sourceFiles(SRC)) {
      const rel = relative(SRC, file);
      if (rel === THIS_FILE) continue;
      const lines = readFileSync(file, "utf8").split("\n");
      for (const [i, line] of lines.entries()) {
        for (const { pattern, what } of FORBIDDEN) {
          if (pattern.test(line)) offences.push(`${rel}:${i + 1} uses ${what}`);
        }
      }
    }
    expect(offences).toEqual([]);
  });

  it("does not depend on anything that speaks HTTP", () => {
    const pkg = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8"));
    const named = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
    const remote = named.filter((name) =>
      /^(axios|node-fetch|got|superagent|ky|undici|isomorphic-fetch|cross-fetch|socket\.io|@sentry\/|posthog|mixpanel|amplitude)/.test(
        name,
      ),
    );
    expect(remote).toEqual([]);
  });
});
