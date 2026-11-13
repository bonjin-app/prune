import { describe, expect, it } from "vitest";
import type { DiskNode, DiskStatus } from "@/types/models";
import { capacity, diskForPath, layoutMap, mapEntries } from "./storage";

const disk = (mountPoint: string, isPrimary = false): DiskStatus => ({
  name: mountPoint,
  mountPoint,
  isPrimary,
  isRemovable: false,
  fileSystem: "apfs",
  totalBytes: 100,
  availableBytes: 12,
});
const folder = (name: string, sizeBytes: number): DiskNode => ({
  name,
  path: `/root/${name}`,
  sizeBytes,
  fileCount: 1,
  dirCount: 0,
  childDirs: 0,
  ownFileBytes: sizeBytes,
});

describe("volume capacity", () => {
  it("uses the primary macOS data volume through firmlinks", () => {
    const primary = disk("/System/Volumes/Data", true);
    expect(diskForPath([disk("/"), primary], "/Users/me/Documents")).toBe(primary);
  });
  it("matches the longest volume path at a boundary", () => {
    const primary = disk("/", true);
    const external = disk("/Volumes/Data");
    const nested = disk("/Volumes/Data/Archive");
    expect(diskForPath([primary, external, nested], "/Volumes/Data/Archive/files")).toBe(nested);
    expect(diskForPath([primary, external], "/Volumes/Database")).toBe(primary);
    expect(diskForPath([primary, external], "/Volumes/Data/")).toBe(external);
  });
  it("normalizes Windows drive separators and case", () => {
    const secondary = disk("D:\\");
    expect(diskForPath([disk("C:\\", true), secondary], "d:\\Projects\\app")).toBe(secondary);
    expect(diskForPath([], "/Users/me")).toBeUndefined();
  });
  it("clamps impossible readings and avoids division by zero", () => {
    expect(capacity(disk("/"))).toEqual({ total: 100, available: 12, used: 88, percent: 88 });
    expect(capacity({ ...disk("/"), availableBytes: 200 }).percent).toBe(0);
    expect(capacity({ ...disk("/"), availableBytes: -1 }).percent).toBe(100);
    expect(capacity({ ...disk("/"), totalBytes: 0 }).percent).toBe(0);
  });
});

describe("folder map", () => {
  it("aggregates small folders without losing bytes", () => {
    const children = [
      folder("main", 900),
      ...Array.from({ length: 20 }, (_, i) => folder(`small${i}`, 2)),
    ];
    const entries = mapEntries(children, 60);
    expect(entries.map((entry) => entry.name)).toEqual(["main", "Files here", "Other folders"]);
    expect(entries.reduce((sum, entry) => sum + entry.bytes, 0)).toBe(1000);
    expect(entries.find((entry) => entry.kind === "other")?.path).toBeUndefined();
  });
  it("keeps at most ten direct folder targets and does not mutate input", () => {
    const children = Array.from({ length: 20 }, (_, i) => folder(`folder${i}`, i + 10));
    const order = children.map((child) => child.name);
    expect(mapEntries(children, 0).filter((entry) => entry.kind === "folder")).toHaveLength(10);
    expect(children.map((child) => child.name)).toEqual(order);
  });
  it("conserves exact areas, stays in bounds, and never overlaps", () => {
    const entries = mapEntries(
      [folder("a", 500), folder("b", 200), folder("c", 100), folder("d", 100)],
      100,
    );
    const tiles = layoutMap(entries);
    expect(tiles.reduce((sum, tile) => sum + tile.width * tile.height, 0)).toBeCloseTo(10000);
    for (const tile of tiles) {
      expect(tile.x).toBeGreaterThanOrEqual(0);
      expect(tile.y).toBeGreaterThanOrEqual(0);
      expect(tile.x + tile.width).toBeLessThanOrEqual(100.00001);
      expect(tile.y + tile.height).toBeLessThanOrEqual(100.00001);
      expect(tile.width * tile.height).toBeCloseTo((tile.bytes / 1000) * 10000);
      for (const other of tiles.filter((other) => other.key !== tile.key)) {
        const overlapX =
          Math.min(tile.x + tile.width, other.x + other.width) - Math.max(tile.x, other.x);
        const overlapY =
          Math.min(tile.y + tile.height, other.y + other.height) - Math.max(tile.y, other.y);
        expect(overlapX <= 0.00001 || overlapY <= 0.00001).toBe(true);
      }
    }
  });
  it("handles empty, zero-size and single-entry folders", () => {
    expect(layoutMap([])).toEqual([]);
    expect(mapEntries([folder("empty", 0)], 0)).toEqual([]);
    expect(layoutMap(mapEntries([], 50))).toMatchObject([{ x: 0, y: 0, width: 100, height: 100 }]);
  });
});
