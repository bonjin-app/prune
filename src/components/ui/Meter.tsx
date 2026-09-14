import { cn } from "@/lib/cn";

/** Horizontal usage bar. `value` in 0–100. */
export function Meter({
  value,
  className,
  tone = "auto",
  height = 6,
}: {
  value: number;
  className?: string;
  tone?: "auto" | "accent" | "info" | "warn" | "danger";
  height?: number;
}) {
  const v = Math.max(0, Math.min(100, value));
  const t = tone === "auto" ? (v >= 90 ? "danger" : v >= 75 ? "warn" : "accent") : tone;
  const color = {
    accent: "bg-accent",
    info: "bg-info",
    warn: "bg-warn",
    danger: "bg-danger",
  }[t];
  return (
    <div
      className={cn(
        "w-full overflow-hidden rounded-full bg-black/[0.07] dark:bg-white/[0.08]",
        className,
      )}
      style={{ height }}
      role="meter"
      aria-valuenow={Math.round(v)}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={cn("h-full rounded-full transition-[width] duration-500", color)}
        style={{ width: `${v}%` }}
      />
    </div>
  );
}
