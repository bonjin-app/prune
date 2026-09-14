import type { ReactNode } from "react";

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border border-line bg-surface px-1 font-sans text-[10px] leading-4 text-fg-faint">
      {children}
    </kbd>
  );
}
