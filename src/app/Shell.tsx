import { TopBar } from "./TopBar";
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
    <div className="flex h-full w-full flex-col bg-bg">
      <TopBar />
      <main className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
        <div key={view} className="fade-in min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-[1080px]">
            <CurrentView />
          </div>
        </div>
      </main>
      <PreviewDialog />
      <ResultDialog />
      <ErrorToast />
    </div>
  );
}
