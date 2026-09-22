import { Search, Settings } from "lucide-react";
import { cn } from "@/lib/cn";
import { isTauri } from "@/lib/tauri";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { ROUTES, SETTINGS_ROUTE, type Route } from "./routes";

/**
 * Where you are, along the top.
 *
 * A column of labelled destinations down the left is how an administration console is laid
 * out, and it makes every screen feel like a page in a site. A utility is a thing you point at
 * a machine: one place to stand, the rest within reach but out of the way. So navigation is a
 * thin strip that never competes with what is underneath it.
 */
function Tab({ route }: { route: Route }) {
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const active = view === route.id;
  return (
    <button
      type="button"
      onClick={() => setView(route.id)}
      aria-current={active ? "page" : undefined}
      title={`${route.label} · ⌘${route.shortcut}`}
      className={cn(
        "relative h-8 rounded-md px-2.5 text-[12.5px] transition-colors",
        active ? "text-fg" : "text-fg-muted hover:text-fg",
      )}
    >
      {route.label}
      {active && (
        <span className="absolute inset-x-2.5 -bottom-[7px] h-[2px] rounded-full bg-accent" />
      )}
    </button>
  );
}

export function TopBar() {
  const setPaletteOpen = useUi((s) => s.setPaletteOpen);
  const view = useUi((s) => s.view);
  const setView = useUi((s) => s.setView);
  const platform = useSystem((s) => s.meta?.platform);
  const hostname = useSystem((s) => s.info?.hostname);
  // Room for the traffic lights, which sit over the window on macOS.
  const macTitlebar = isTauri && platform !== "windows";

  return (
    <header
      className={cn(
        "relative z-10 flex shrink-0 items-center gap-1 border-b border-line bg-bg px-3",
        macTitlebar ? "h-[52px] pt-3 pl-[88px]" : "h-[46px]",
      )}
      data-tauri-drag-region
    >
      <span className="mr-3 flex items-center gap-2 text-[13px] font-semibold tracking-tight">
        <Leaf />
        Prune
      </span>

      <nav aria-label="Sections" className="flex items-center gap-0.5">
        {ROUTES.map((r) => (
          <Tab key={r.id} route={r} />
        ))}
      </nav>

      <div className="flex-1" />

      {hostname && (
        <span className="mr-1 hidden text-[11.5px] text-fg-faint sm:block">{hostname}</span>
      )}
      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        aria-label="Search or run a command"
        title="Search or run…  ⌘K"
        className="flex h-8 w-8 items-center justify-center rounded-md text-fg-faint hover:bg-surface-2 hover:text-fg"
      >
        <Search size={15} />
      </button>
      <button
        type="button"
        onClick={() => setView(SETTINGS_ROUTE.id)}
        aria-label="Settings"
        aria-current={view === SETTINGS_ROUTE.id ? "page" : undefined}
        title="Settings  ⌘,"
        className={cn(
          "flex h-8 w-8 items-center justify-center rounded-md hover:bg-surface-2 hover:text-fg",
          view === SETTINGS_ROUTE.id ? "text-fg" : "text-fg-faint",
        )}
      >
        <Settings size={15} />
      </button>
    </header>
  );
}

/** The mark: a cut branch putting out new growth. */
function Leaf() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M8 14V6.5"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        className="text-fg-faint"
      />
      <path
        d="M8 7.5C8 4.9 10.1 2.8 12.7 2.8c0 2.6-2.1 4.7-4.7 4.7Z"
        fill="currentColor"
        className="text-accent"
      />
      <path
        d="M8 10.5C8 8.8 6.6 7.4 4.9 7.4c0 1.7 1.4 3.1 3.1 3.1Z"
        fill="currentColor"
        className="text-accent/55"
      />
    </svg>
  );
}
