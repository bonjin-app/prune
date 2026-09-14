import { describe, expect, it } from "vitest";
import { abbreviatePath, formatBytes, formatDuration, splitBytes } from "./format";

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
    expect(abbreviatePath("/tmp/x", "/Users/dev")).toBe("/tmp/x");
    expect(abbreviatePath("/tmp/x", null)).toBe("/tmp/x");
  });
});
