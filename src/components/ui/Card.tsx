import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/cn";

export function Card({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "rounded-xl border border-line/70 bg-surface",
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
    <div className="flex items-start justify-between gap-3 px-4 pt-4 pb-2.5">
      <div className="min-w-0">
        <h3 className="text-[13.5px] font-semibold tracking-tight">{title}</h3>
        {subtitle && <p className="mt-0.5 text-[12px] text-fg-muted">{subtitle}</p>}
      </div>
      {action}
    </div>
  );
}
