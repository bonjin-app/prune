import { useEffect } from "react";
import { ROUTES, SETTINGS_ROUTE } from "./app/routes";
import { ErrorBoundary } from "./app/ErrorBoundary";
import { Shell } from "./app/Shell";
import { CommandPalette } from "./features/palette/CommandPalette";
import { bindAppsEvents } from "./stores/apps";
import { bindDiskEvents } from "./stores/disk";
import { bindScanEvents, useScan } from "./stores/scan";
import { useSystem } from "./stores/system";
import { useUi } from "./stores/ui";

export default function App() {
  const loadStatic = useSystem((s) => s.loadStatic);
  const loadProviders = useScan((s) => s.loadProviders);
  const setPaletteOpen = useUi((s) => s.setPaletteOpen);

  useEffect(() => {
    void loadStatic();
    void loadProviders();
    const unbinders: (() => void)[] = [];
    void bindScanEvents().then((u) => unbinders.push(u));
    void bindDiskEvents().then((u) => unbinders.push(u));
    void bindAppsEvents().then((u) => unbinders.push(u));
    return () => unbinders.forEach((u) => u());
  }, [loadStatic, loadProviders]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      const key = e.key.toLowerCase();
      if (key === "k") {
        e.preventDefault();
        setPaletteOpen(!useUi.getState().paletteOpen);
        return;
      }
      // Cmd/Ctrl + digit jumps between views; Cmd/Ctrl + comma opens Settings.
      const route = [...ROUTES, SETTINGS_ROUTE].find((r) => r.shortcut === e.key);
      if (route) {
        e.preventDefault();
        useUi.getState().setPaletteOpen(false);
        useUi.getState().setView(route.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setPaletteOpen]);

  return (
    <ErrorBoundary>
      <Shell />
      <CommandPalette />
    </ErrorBoundary>
  );
}
