import { cn } from "@/lib/cn";
import { useScan } from "@/stores/scan";
import type { DeleteMode } from "@/types/models";

export function DeleteModeToggle() {
  const mode = useScan((s) => s.deleteMode);
  const setMode = useScan((s) => s.setDeleteMode);
  const opt = (value: DeleteMode, label: string) => (
    <button
      type="button"
      onClick={() => setMode(value)}
      aria-pressed={mode === value}
      className={cn(
        "h-6 rounded-[5px] px-2 text-[11.5px] font-medium transition-colors",
        mode === value
          ? value === "permanent"
            ? "bg-danger text-white"
            : "bg-surface text-fg shadow-sm"
          : "text-fg-muted hover:text-fg",
      )}
    >
      {label}
    </button>
  );
  return (
    <div
      className="flex items-center gap-0.5 rounded-md border border-line bg-surface-2 p-0.5"
      title="How selected items are removed"
    >
      {opt("trash", "Move to Trash")}
      {opt("permanent", "Delete permanently")}
    </div>
  );
}
