import { cn } from "@/lib/cn";
import { isTauri } from "@/lib/tauri";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { ROUTES, SETTINGS_ROUTE, type Route } from "./routes";
import { Kbd } from "@/components/ui/Kbd";
import { Search } from "lucide-react";

function NavItem({ route }: { route: Route }) {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const active = view === route.id;
  const Icon = route.icon;
  return (
    <button
      type="button"
      onClick={() => setView(route.id)}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[13px] transition-colors",
        active
          ? "bg-surface text-fg shadow-[0_1px_0_rgba(0,0,0,0.04)] dark:bg-surface-2"
          : "text-fg-muted hover:bg-black/[0.04] hover:text-fg dark:hover:bg-white/[0.05]",
      )}
    >
      <Icon
        size={15}
        strokeWidth={1.9}
        className={cn(active ? "text-accent" : "text-fg-faint group-hover:text-fg-muted")}
      />
      <span className="flex-1 truncate">{route.label}</span>
      {!route.ready && (
        <span className="rounded-sm border border-line px-1 text-[10px] leading-4 text-fg-faint">
          soon
        </span>
      )}
    </button>
  );
}

export function Sidebar() {
  const platform = useSystem((s) => s.meta?.platform);
  const setPaletteOpen = useUi((s) => s.setPaletteOpen);
  const macTitlebar = isTauri && platform !== "windows";
  return (
    <aside
      className="flex h-full w-[220px] shrink-0 flex-col border-r border-line bg-bg-sidebar"
      data-tauri-drag-region
    >
      <div className={cn("px-3", macTitlebar ? "pt-11" : "pt-3")} data-tauri-drag-region>
        <div className="flex items-center gap-2 px-1.5" data-tauri-drag-region>
          <PruneMark />
          <span className="text-[14px] font-semibold tracking-tight">Prune</span>
        </div>
      </div>

      <div className="px-3 pt-4">
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="flex w-full items-center gap-2 rounded-md border border-line bg-surface/60 px-2.5 py-1.5 text-left text-[12px] text-fg-faint hover:border-line-strong hover:text-fg-muted"
        >
          <Search size={13} />
          <span className="flex-1">Search or run…</span>
          <Kbd>⌘K</Kbd>
        </button>
      </div>

      <nav className="flex flex-1 flex-col gap-0.5 px-3 pt-4">
        {ROUTES.map((r) => (
          <NavItem key={r.id} route={r} />
        ))}
      </nav>

      <div className="px-3 pb-3">
        <NavItem route={SETTINGS_ROUTE} />
      </div>
    </aside>
  );
}

export function PruneMark({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden>
      <path d="M12 21V8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <path
        d="M12 14c1.5-1.4 3-2 5-2.6"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path d="M17 11.4c1.6-2.2 3.6-2.6 5-2.4-.4 2.6-2.2 4.4-5 4.4z" fill="var(--accent)" />
      <path d="M12 8c-.4-2.6 1-4.6 3.6-5 .2 2.6-1 4.6-3.6 5z" fill="var(--accent)" />
      <path d="M12 11.5 9.5 10.3" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
      <path
        d="m7.2 7.6 2.6 3.4M9.8 7.6 7.2 11"
        stroke="var(--danger)"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}
