import { describe, expect, it } from "vitest";
import {
  abbreviatePath,
  formatBytes,
  formatDateTime,
  formatDuration,
  formatRelative,
  plural,
  splitBytes,
} from "./format";

describe("formatBytes", () => {
  it("uses decimal units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1000)).toBe("1.00 KB");
    expect(formatBytes(12_800_000_000)).toBe("12.8 GB");
    expect(formatBytes(382_000_000_000)).toBe("382 GB");
    expect(formatBytes(1_000_000_000_000)).toBe("1.00 TB");
  });

  it("handles invalid input", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.NaN)).toBe("—");
  });

  it("splits number and unit", () => {
    expect(splitBytes(8_400_000_000)).toEqual(["8.40", "GB"]);
  });

  it("splits something with no unit without inventing one", () => {
    expect(splitBytes(Number.NaN)).toEqual(["—", ""]);
  });
});

describe("formatDuration", () => {
  it("formats days, hours and minutes", () => {
    expect(formatDuration(90)).toBe("1m");
    expect(formatDuration(3700)).toBe("1h 1m");
    expect(formatDuration(90000)).toBe("1d 1h");
  });
});

describe("abbreviatePath", () => {
  it("replaces the home prefix", () => {
    expect(abbreviatePath("/Users/dev/Library/Caches", "/Users/dev")).toBe("~/Library/Caches");
    expect(abbreviatePath("/Users/dev", "/Users/dev")).toBe("~");
    expect(abbreviatePath("/tmp/x", "/Users/dev")).toBe("/tmp/x");
    expect(abbreviatePath("/tmp/x", null)).toBe("/tmp/x");
  });

  it("only at a path boundary", () => {
    // Another account whose name happens to start with this one's.
    expect(abbreviatePath("/Users/developer/Library", "/Users/dev")).toBe(
      "/Users/developer/Library",
    );
    expect(abbreviatePath("/Users/dev/x", "/Users/dev/")).toBe("~/x");
    expect(abbreviatePath("C:\\Users\\dev\\AppData", "C:\\Users\\dev")).toBe("~\\AppData");
    expect(abbreviatePath("/anything", "/")).toBe("/anything");
  });
});

describe("plural", () => {
  it("agrees with its count", () => {
    expect(plural(0, "file")).toBe("0 files");
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(2, "item")).toBe("2 items");
    expect(plural(43_750, "file")).toMatch(/^43.750 files$/);
    expect(plural(1, "process", "processes")).toBe("1 process");
    expect(plural(3, "process", "processes")).toBe("3 processes");
  });
});

describe("formatRelative", () => {
  const now = Date.parse("2026-10-03T12:00:00Z");
  const ago = (ms: number) => new Date(now - ms).toISOString();
  const MIN = 60_000;
  const DAY = 24 * 60 * MIN;

  it("keeps the same short shape at every age", () => {
    expect(formatRelative(ago(10_000), now)).toBe("just now");
    expect(formatRelative(ago(5 * MIN), now)).toBe("5m ago");
    expect(formatRelative(ago(3 * 60 * MIN), now)).toBe("3h ago");
    expect(formatRelative(ago(26 * DAY), now)).toBe("26d ago");
    expect(formatRelative(ago(29 * DAY), now)).toBe("29d ago");
    expect(formatRelative(ago(31 * DAY), now)).toBe("1mo ago");
    expect(formatRelative(ago(70 * DAY), now)).toBe("2mo ago");
    expect(formatRelative(ago(364 * DAY), now)).toBe("12mo ago");
    expect(formatRelative(ago(400 * DAY), now)).toBe("1y ago");
    expect(formatRelative(ago(3 * 366 * DAY), now)).toBe("3y ago");
  });

  it("never falls back to a locale date", () => {
    // What it used to print past thirty days, on a Korean system.
    const old = formatRelative(ago(40 * DAY), now);
    expect(old).toMatch(/^\d+mo ago$/);
    expect([...old].every((c) => c.charCodeAt(0) < 128)).toBe(true);
  });

  it("treats a timestamp in the future as now, not as negative time", () => {
    expect(formatRelative(new Date(now + 5 * MIN).toISOString(), now)).toBe("just now");
  });

  it("returns what it cannot parse unchanged", () => {
    expect(formatRelative("not a date", now)).toBe("not a date");
  });
});

describe("formatDateTime", () => {
  it("is the same in every locale", () => {
    const local = new Date(2026, 8, 4, 3, 36);
    expect(formatDateTime(local.toISOString())).toBe("2026-09-04 03:36");
  });

  it("returns what it cannot parse unchanged", () => {
    expect(formatDateTime("nope")).toBe("nope");
  });
});
