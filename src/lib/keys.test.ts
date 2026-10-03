import { describe, expect, it } from "vitest";
import { shortcutAria, shortcutLabel } from "./keys";

describe("shortcutLabel", () => {
  it("uses the key the keyboard actually has", () => {
    expect(shortcutLabel("macos", "1")).toBe("⌘1");
    expect(shortcutLabel("macos", "k")).toBe("⌘K");
    // Windows keyboards have no ⌘; this is what every hint used to say there.
    expect(shortcutLabel("windows", "k")).toBe("Ctrl+K");
    expect(shortcutLabel("windows", ",")).toBe("Ctrl+,");
  });

  it("falls back to the Mac spelling before the platform is known", () => {
    expect(shortcutLabel(undefined, "2")).toBe("⌘2");
  });
});

describe("shortcutAria", () => {
  it("spells the modifier out for assistive technology", () => {
    expect(shortcutAria("macos", "1")).toBe("Meta+1");
    expect(shortcutAria("windows", "1")).toBe("Control+1");
  });
});
