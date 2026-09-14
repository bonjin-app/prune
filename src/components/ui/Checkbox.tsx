import { Check, Minus } from "lucide-react";
import { cn } from "@/lib/cn";

export function Checkbox({
  checked,
  indeterminate,
  onChange,
  disabled,
  label,
  className,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  label?: string;
  className?: string;
}) {
  const on = checked || indeterminate;
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? "mixed" : checked}
      aria-label={label}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={cn(
        "flex h-[15px] w-[15px] shrink-0 items-center justify-center rounded-[4px] border transition-colors",
        on
          ? "border-accent bg-accent text-accent-fg"
          : "border-line-strong bg-surface hover:border-fg-faint",
        disabled && "cursor-not-allowed opacity-40",
        className,
      )}
    >
      {indeterminate ? (
        <Minus size={11} strokeWidth={3} />
      ) : checked ? (
        <Check size={11} strokeWidth={3} />
      ) : null}
    </button>
  );
}
