import { useEffect, useMemo } from "react";
import { FolderOpen, Power, RefreshCw, ShieldAlert } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/EmptyState";
import { PageHeader } from "@/components/ui/PageHeader";
import { Spinner } from "@/components/ui/Spinner";
import { Switch } from "@/components/ui/Switch";
import { cn } from "@/lib/cn";
import { abbreviatePath } from "@/lib/format";
import { backend, isTauri } from "@/lib/tauri";
import { useStartup } from "@/stores/startup";
import { useSystem } from "@/stores/system";
import {
  STARTUP_SOURCE_LABEL,
  STARTUP_TRIGGER_LABEL,
  type StartupItem,
  type StartupScope,
} from "@/types/models";

export function StartupView() {
  const items = useStartup((s) => s.items);
  const loading = useStartup((s) => s.loading);
  const load = useStartup((s) => s.load);
  const platform = useSystem((s) => s.meta?.platform);

  useEffect(() => {
    if (items.length === 0 && !loading) void load();
  }, [items.length, loading, load]);

  const groups = useMemo(() => {
    const by = (scope: StartupScope) => items.filter((i) => i.scope === scope);
    return [
      { scope: "user" as const, title: "Your account", items: by("user") },
      { scope: "system" as const, title: "All users", items: by("system") },
    ].filter((g) => g.items.length > 0);
  }, [items]);

  const enabled = items.filter((i) => i.enabled).length;

  return (
    <div className="pb-8">
      <PageHeader
        title="Startup"
        description="Programs that launch by themselves. Disabling one leaves it installed and takes effect at your next login."
        actions={
          <Button variant="secondary" size="sm" loading={loading} onClick={() => void load()}>
            <RefreshCw size={12} /> Refresh
          </Button>
        }
      />

      <div className="px-7">
        {items.length === 0 && !loading ? (
          <EmptyState
            icon={Power}
            title="Nothing starts by itself"
            description="Prune found no launch agents, daemons or startup entries for this account."
          />
        ) : (
          <>
            <div className="pb-3 text-[12px] text-fg-muted tnum">
              {enabled} of {items.length} enabled
            </div>
            <div className="flex flex-col gap-3">
              {groups.map((g) => (
                <section
                  key={g.scope}
                  className="overflow-hidden rounded-lg border border-line bg-surface"
                >
                  <header className="flex items-center gap-2 border-b border-line px-3 py-2">
                    <h3 className="text-[12.5px] font-semibold">{g.title}</h3>
                    <span className="text-[11px] text-fg-faint tnum">{g.items.length}</span>
                    {g.scope === "system" && (
                      <span className="ml-auto flex items-center gap-1 text-[11px] text-fg-faint">
                        <ShieldAlert size={11} /> read-only without administrator rights
                      </span>
                    )}
                  </header>
                  {g.items.map((item) => (
                    <Row key={item.id} item={item} />
                  ))}
                </section>
              ))}
            </div>
            {platform === "macos" && (
              <p className="mt-3 text-[11.5px] text-fg-faint">
                Applications that register themselves through macOS Login Items are managed by the
                system and appear in System Settings, not here.
              </p>
            )}
          </>
        )}
        {loading && items.length === 0 && (
          <div className="flex items-center justify-center gap-2 py-16 text-[12.5px] text-fg-muted">
            <Spinner size={14} className="text-accent" /> Reading startup items…
          </div>
        )}
      </div>
    </div>
  );
}

function Row({ item }: { item: StartupItem }) {
  const setEnabled = useStartup((s) => s.setEnabled);
  const busy = useStartup((s) => s.pending.has(item.id));
  const home = useSystem((s) => s.info?.homeDir);
  const target = item.command ?? item.path;

  return (
    <div
      className={cn(
        "flex items-center gap-3 border-t border-line/60 px-3 py-2 first:border-t-0",
        !item.enabled && "opacity-60",
      )}
    >
      <Switch
        checked={item.enabled}
        disabled={!item.canToggle}
        busy={busy}
        onChange={(next) => void setEnabled(item.id, next)}
        label={`${item.enabled ? "Disable" : "Enable"} ${item.name}`}
      />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[12.5px] font-medium">{item.name}</span>
          <span className="shrink-0 rounded-sm bg-surface-2 px-1 text-[10.5px] text-fg-muted">
            {STARTUP_TRIGGER_LABEL[item.trigger]}
          </span>
          {!item.canToggle && (
            <span title={item.reason} className="shrink-0 text-fg-faint">
              <ShieldAlert size={11} />
            </span>
          )}
        </div>
        <div className="truncate font-mono text-[11px] text-fg-faint">
          {abbreviatePath(target, home)}
        </div>
      </div>
      <div className="w-[96px] shrink-0 text-right text-[11px] text-fg-faint">
        {STARTUP_SOURCE_LABEL[item.source] ?? item.source}
      </div>
      {isTauri && (
        <button
          type="button"
          onClick={() => void backend.fsReveal(item.path)}
          className="rounded p-1 text-fg-faint hover:bg-surface-2 hover:text-fg"
          title="Reveal"
        >
          <FolderOpen size={13} />
        </button>
      )}
    </div>
  );
}
