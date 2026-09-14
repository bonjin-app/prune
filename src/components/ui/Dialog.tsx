import { useEffect, useRef, type ReactNode } from "react";
import { cn } from "@/lib/cn";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * A modal dialog that keeps the keyboard inside it.
 *
 * Prune's dialogs are where destructive choices are confirmed, so focus must start in the
 * dialog, stay in the dialog while it is open, and return to whatever opened it afterwards —
 * otherwise a keyboard user can tab into the page behind the dialog and act on it blind.
 */
export function Dialog({
  open,
  onClose,
  children,
  className,
  closeOnBackdrop = true,
  label,
}: {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  className?: string;
  closeOnBackdrop?: boolean;
  /** Accessible name, announced when the dialog opens. */
  label?: string;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const restoreTo = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    restoreTo.current = document.activeElement as HTMLElement | null;

    // Move focus into the dialog. Prefer the first control; fall back to the panel itself.
    const first = panel.current?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? panel.current)?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key !== "Tab" || !panel.current) return;
      // The selector already excludes disabled controls and tabindex="-1"; deliberately no
      // visibility filtering, because layout-based checks (offsetParent, getClientRects) are
      // unreliable inside a fixed-position overlay and would silently break the trap.
      const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
      if (items.length === 0) {
        e.preventDefault();
        panel.current.focus();
        return;
      }
      const first = items[0]!;
      const last = items[items.length - 1]!;
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === panel.current)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      restoreTo.current?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/30 p-6 backdrop-blur-[2px] dark:bg-black/50"
      onMouseDown={(e) => {
        if (closeOnBackdrop && e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        className={cn(
          "fade-in w-full max-w-[560px] overflow-hidden rounded-xl border border-line bg-surface shadow-2xl outline-none",
          className,
        )}
      >
        {children}
      </div>
    </div>
  );
}
