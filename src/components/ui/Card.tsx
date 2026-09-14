import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";

export function Card({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "rounded-lg border border-line bg-surface shadow-[0_1px_2px_rgba(0,0,0,0.03)]",
        className,
      )}
      {...rest}
    />
  );
}

export function CardHeader({
  title,
  subtitle,
  action,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-3 px-4 pt-3.5 pb-2">
      <div className="min-w-0">
        <h3 className="text-[13px] font-semibold tracking-tight">{title}</h3>
        {subtitle && <p className="mt-0.5 text-[12px] text-fg-muted">{subtitle}</p>}
      </div>
      {action}
    </div>
  );
}
