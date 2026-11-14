import type { DiskNode, DiskStatus } from "@/types/models";

function normalized(path: string) {
  const p = path.replaceAll("\\", "/").replace(/\/+$/, "");
  return /^[A-Za-z]:/.test(p) ? p.toLowerCase() : p;
}

/** Match a volume at a path boundary, not e.g. /Volumes/Data against /Volumes/Database. */
export function diskForPath(disks: DiskStatus[], path: string): DiskStatus | undefined {
  const target = normalized(path);
  const primary = disks.find((disk) => disk.isPrimary) ?? disks[0];
  const matches = disks.filter((disk) => {
    const mount = normalized(disk.mountPoint);
    return mount && (target === mount || target.startsWith(`${mount}/`));
  });
  matches.sort((a, b) => b.mountPoint.length - a.mountPoint.length);
  // macOS exposes the data volume through firmlinks; /Users is not prefixed by its mount.
  return matches[0] ?? primary;
}

export function capacity(disk: DiskStatus) {
  const total = Math.max(0, disk.totalBytes);
  const available = Math.min(total, Math.max(0, disk.availableBytes));
  const used = total - available;
  return { total, available, used, percent: total > 0 ? (used / total) * 100 : 0 };
}

export interface MapEntry {
  key: string;
  name: string;
  bytes: number;
  path?: string;
  kind: "folder" | "files" | "other";
}

/** Keep small folders legible in the list instead of creating unusable tiny map targets. */
export function mapEntries(children: DiskNode[], ownFileBytes: number): MapEntry[] {
  const sorted = children
    .filter((child) => child.sizeBytes > 0)
    .sort((a, b) => b.sizeBytes - a.sizeBytes);
  const total = sorted.reduce((sum, child) => sum + child.sizeBytes, ownFileBytes);
  const entries: MapEntry[] = [];
  let other = 0;
  for (const child of sorted) {
    if (entries.length < 10 && child.sizeBytes / total >= 0.015) {
      entries.push({
        key: child.path,
        name: child.name,
        bytes: child.sizeBytes,
        path: child.path,
        kind: "folder",
      });
    } else {
      other += child.sizeBytes;
    }
  }
  if (ownFileBytes > 0)
    entries.push({ key: "own", name: "Files here", bytes: ownFileBytes, kind: "files" });
  if (other > 0) entries.push({ key: "other", name: "Other folders", bytes: other, kind: "other" });
  return entries.sort((a, b) => b.bytes - a.bytes);
}

export interface MapTile extends MapEntry {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A binary treemap with exact area conservation and no pixel-sized SVG text. */
export function layoutMap(entries: MapEntry[]): MapTile[] {
  const positive = entries.filter((entry) => entry.bytes > 0);
  function split(
    items: MapEntry[],
    x: number,
    y: number,
    width: number,
    height: number,
  ): MapTile[] {
    if (items.length === 0) return [];
    const first = items[0];
    if (!first) return [];
    if (items.length === 1) return [{ ...first, x, y, width, height }];
    const total = items.reduce((sum, item) => sum + item.bytes, 0);
    let count = 1;
    let left = first.bytes;
    while (count < items.length - 1) {
      const next = items[count];
      if (!next || Math.abs(total / 2 - left - next.bytes) >= Math.abs(total / 2 - left)) break;
      left += next.bytes;
      count += 1;
    }
    const ratio = left / total;
    return width >= height
      ? [
          ...split(items.slice(0, count), x, y, width * ratio, height),
          ...split(items.slice(count), x + width * ratio, y, width * (1 - ratio), height),
        ]
      : [
          ...split(items.slice(0, count), x, y, width, height * ratio),
          ...split(items.slice(count), x, y + height * ratio, width, height * (1 - ratio)),
        ];
  }
  return split(positive, 0, 0, 100, 100);
}
