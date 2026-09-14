import { Command } from "cmdk";
import { HardDrive, Moon, RefreshCw, Sun, SunMoon } from "lucide-react";
import { useCallback } from "react";
import { Dialog } from "@/components/ui/Dialog";
import { Kbd } from "@/components/ui/Kbd";
import { ROUTES, SETTINGS_ROUTE } from "@/app/routes";
import { useDisk } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useUi } from "@/stores/ui";
import { CLEANER_CATEGORIES, DEVELOPER_CATEGORIES } from "@/types/models";

export function CommandPalette() {
  const open = useUi((s) => s.paletteOpen);
  const setOpen = useUi((s) => s.setPaletteOpen);
  const setView = useUi((s) => s.setView);
  const setTheme = useUi((s) => s.setTheme);
  const providers = useScan((s) => s.providers);
  const startScan = useScan((s) => s.startScan);
  const scanning = useScan((s) => s.scanning);
  const startDisk = useDisk((s) => s.startScan);
  const diskScanning = useDisk((s) => s.scanning);

  const run = useCallback(
    (fn: () => void) => () => {
      setOpen(false);
      fn();
    },
    [setOpen],
  );

  const idsFor = (cats: string[]) =>
    providers.filter((p) => cats.includes(p.category)).map((p) => p.id);

  return (
    <Dialog open={open} onClose={() => setOpen(false)} className="max-w-[520px]">
      <Command label="Command palette" className="text-[13px]">
        <div className="flex items-center gap-2 border-b border-line px-3">
          <Command.Input
            autoFocus
            placeholder="Type a command or search…"
            className="h-11 flex-1 bg-transparent text-[14px] outline-none placeholder:text-fg-faint"
          />
          <Kbd>esc</Kbd>
        </div>
        <Command.List className="max-h-[360px] overflow-y-auto p-1.5 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:text-fg-faint">
          <Command.Empty className="px-3 py-6 text-center text-fg-faint">No results.</Command.Empty>

          <Command.Group heading="Actions">
            <Item
              onSelect={run(() => {
                setView("cleaner");
                void startScan(idsFor(CLEANER_CATEGORIES));
              })}
              disabled={scanning}
            >
              <RefreshCw size={14} /> Clean caches
            </Item>
            <Item
              onSelect={run(() => {
                setView("developer");
                void startScan(idsFor(DEVELOPER_CATEGORIES));
              })}
              disabled={scanning}
            >
              <RefreshCw size={14} /> Scan developer files
            </Item>
            <Item
              onSelect={run(() => {
                setView("dashboard");
                void startScan();
              })}
              disabled={scanning}
            >
              <RefreshCw size={14} /> Scan everything
            </Item>
            <Item
              onSelect={run(() => {
                setView("disk");
                void startDisk();
              })}
              disabled={diskScanning}
            >
              <HardDrive size={14} /> Analyze disk usage
            </Item>
            <Item
              onSelect={run(() => {
                setView("disk");
                useDisk.getState().setTab("large");
                if (!useDisk.getState().summary) void startDisk();
              })}
              disabled={diskScanning}
            >
              <HardDrive size={14} /> Find large files
            </Item>
          </Command.Group>

          <Command.Group heading="Go to">
            {[...ROUTES, SETTINGS_ROUTE].map((r) => (
              <Item key={r.id} onSelect={run(() => setView(r.id))}>
                <r.icon size={14} /> {r.label}
                <span className="ml-auto text-[11px] text-fg-faint">⌘{r.shortcut}</span>
              </Item>
            ))}
          </Command.Group>

          <Command.Group heading="Appearance">
            <Item onSelect={run(() => setTheme("system"))}>
              <SunMoon size={14} /> Use system theme
            </Item>
            <Item onSelect={run(() => setTheme("light"))}>
              <Sun size={14} /> Light theme
            </Item>
            <Item onSelect={run(() => setTheme("dark"))}>
              <Moon size={14} /> Dark theme
            </Item>
          </Command.Group>
        </Command.List>
      </Command>
    </Dialog>
  );
}

function Item({
  children,
  onSelect,
  disabled,
}: {
  children: React.ReactNode;
  onSelect: () => void;
  disabled?: boolean;
}) {
  return (
    <Command.Item
      onSelect={onSelect}
      disabled={disabled}
      className="flex cursor-default items-center gap-2.5 rounded-md px-2.5 py-2 text-fg data-[disabled=true]:opacity-40 data-[selected=true]:bg-accent-soft data-[selected=true]:text-fg [&_svg]:text-fg-faint data-[selected=true]:[&_svg]:text-accent"
    >
      {children}
    </Command.Item>
  );
}
