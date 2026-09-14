import { cn } from "@/lib/cn";
import { RISK_LABEL, type RiskLevel } from "@/types/models";

const styles: Record<RiskLevel, string> = {
  safe: "bg-accent-soft text-accent",
  low: "bg-info-soft text-info",
  medium: "bg-warn-soft text-warn",
  high: "bg-danger-soft text-danger",
  protected: "bg-surface-2 text-fg-faint border border-line",
};

export function RiskBadge({ risk, className }: { risk: RiskLevel; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-[18px] items-center rounded-sm px-1.5 text-[10.5px] font-medium leading-none whitespace-nowrap",
        styles[risk],
        className,
      )}
    >
      {RISK_LABEL[risk]}
    </span>
  );
}
