import { useEffect } from "react";
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
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(!useUi.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setPaletteOpen]);

  return (
    <>
      <Shell />
      <CommandPalette />
    </>
  );
}
