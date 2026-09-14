import { cn } from "@/lib/cn";

export function Switch({
  checked,
  onChange,
  disabled,
  busy,
  label,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  busy?: boolean;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled || busy}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={cn(
        "relative h-[18px] w-[32px] shrink-0 rounded-full transition-colors",
        checked ? "bg-accent" : "bg-black/[0.14] dark:bg-white/[0.16]",
        (disabled || busy) && "cursor-not-allowed opacity-40",
      )}
    >
      <span
        className={cn(
          "absolute top-[2px] h-[14px] w-[14px] rounded-full bg-white shadow-sm transition-[left]",
          checked ? "left-[16px]" : "left-[2px]",
          busy && "animate-pulse",
        )}
      />
    </button>
  );
}
