const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** Decimal units (1 GB = 1,000,000,000 B), matching Finder and Windows Settings. */
export function formatBytes(bytes: number, digits?: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1000) return `${Math.round(bytes)} B`;
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const d = digits ?? (value >= 100 ? 0 : value >= 10 ? 1 : 2);
  return `${value.toFixed(d)} ${UNITS[unit]}`;
}

/** Splits into `[number, unit]` so the UI can style them separately. */
export function splitBytes(bytes: number): [string, string] {
  const s = formatBytes(bytes);
  const i = s.lastIndexOf(" ");
  return i === -1 ? [s, ""] : [s.slice(0, i), s.slice(i + 1)];
}

/**
 * "1 file", "2 files". A count with the wrong noun reads as a bug even when the number is
 * right, and every count in the interface goes through here so none of them can.
 */
export function plural(n: number, one: string, many = `${one}s`): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}

export function formatPercent(value: number, digits = 0): string {
  if (!Number.isFinite(value)) return "—";
  return `${value.toFixed(digits)}%`;
}

export function formatCount(n: number): string {
  return new Intl.NumberFormat().format(n);
}

export function formatDuration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

const pad = (n: number) => String(n).padStart(2, "0");

/**
 * `2026-09-04 03:36`, in local time.
 *
 * Deliberately not the system locale's format: the interface is English, and on a Korean system
 * the locale format is "2026. 09. 04. 오전 03:36" — another language in the middle of an English
 * sentence, and long enough to wrap a table cell onto three lines.
 */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}`
  );
}

/**
 * "3d ago", "2mo ago", "1y ago" — the same short shape at every age.
 *
 * It used to give up after thirty days and print the full date and time, so a list of caches
 * read "26d ago" on one row and a three-line timestamp on the next. A cache two months old is
 * described well enough by "2mo ago"; the exact minute it was last written is noise.
 */
export function formatRelative(iso: string, now: number = Date.now()): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  // A timestamp slightly in the future is clock skew, not time travel.
  const mins = Math.round(Math.max(0, now - then) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}d ago`;
  if (days < 365) return `${Math.max(1, Math.round(days / 30.44))}mo ago`;
  return `${Math.floor(days / 365.25)}y ago`;
}

/**
 * Replaces the home directory prefix with `~` for compact display.
 *
 * Only at a path boundary: with a home of `/Users/dev`, `/Users/developer/x` is someone else's
 * folder, not `~eloper/x`.
 */
export function abbreviatePath(path: string, home: string | null | undefined): string {
  if (!home) return path;
  const base = home.replace(/[/\\]+$/, "");
  if (!base) return path;
  if (path === base) return "~";
  const next = path.charAt(base.length);
  if (path.startsWith(base) && (next === "/" || next === "\\")) {
    return `~${path.slice(base.length)}`;
  }
  return path;
}
