import { Sidebar } from "./Sidebar";
import { useUi } from "@/stores/ui";
import { DashboardView } from "@/features/dashboard/DashboardView";
import { CleanerView } from "@/features/cleaner/CleanerView";
import { DeveloperView } from "@/features/developer/DeveloperView";
import { DiskView } from "@/features/disk/DiskView";
import { UninstallerView } from "@/features/uninstaller/UninstallerView";
import { MonitorView } from "@/features/monitor/MonitorView";
import { StartupView } from "@/features/startup/StartupView";
import { SettingsView } from "@/features/settings/SettingsView";
import { PreviewDialog } from "@/features/cleaner/PreviewDialog";
import { ResultDialog } from "@/features/cleaner/ResultDialog";
import { ErrorToast } from "@/components/ui/ErrorToast";

function CurrentView() {
  const view = useUi((s) => s.view);
  switch (view) {
    case "dashboard":
      return <DashboardView />;
    case "cleaner":
      return <CleanerView />;
    case "developer":
      return <DeveloperView />;
    case "disk":
      return <DiskView />;
    case "uninstaller":
      return <UninstallerView />;
    case "monitor":
      return <MonitorView />;
    case "startup":
      return <StartupView />;
    case "settings":
      return <SettingsView />;
  }
}

export function Shell() {
  const view = useUi((s) => s.view);
  return (
    <div className="flex h-full w-full">
      <Sidebar />
      <main className="relative flex min-w-0 flex-1 flex-col overflow-hidden bg-bg">
        {/* Drag region under the traffic lights on macOS. */}
        <div className="h-3 shrink-0" data-tauri-drag-region />
        <div key={view} className="fade-in min-h-0 flex-1 overflow-y-auto">
          <CurrentView />
        </div>
      </main>
      <PreviewDialog />
      <ResultDialog />
      <ErrorToast />
    </div>
  );
}
