import {
  Activity,
  Code2,
  HardDrive,
  LayoutDashboard,
  type LucideIcon,
  PackageMinus,
  Power,
  Settings,
  Sparkles,
} from "lucide-react";
import type { ViewId } from "@/stores/ui";

export interface Route {
  id: ViewId;
  label: string;
  icon: LucideIcon;
  /** Roadmap phase; views not yet implemented show a placeholder. */
  phase: number;
  ready: boolean;
  shortcut: string;
}

export const ROUTES: Route[] = [
  {
    id: "dashboard",
    label: "Dashboard",
    icon: LayoutDashboard,
    phase: 2,
    ready: true,
    shortcut: "1",
  },
  { id: "cleaner", label: "Cleaner", icon: Sparkles, phase: 3, ready: true, shortcut: "2" },
  {
    id: "uninstaller",
    label: "Uninstaller",
    icon: PackageMinus,
    phase: 6,
    ready: false,
    shortcut: "3",
  },
  { id: "disk", label: "Disk", icon: HardDrive, phase: 5, ready: true, shortcut: "4" },
  { id: "developer", label: "Developer", icon: Code2, phase: 4, ready: true, shortcut: "5" },
  { id: "monitor", label: "Monitor", icon: Activity, phase: 7, ready: true, shortcut: "6" },
  { id: "startup", label: "Startup", icon: Power, phase: 7, ready: false, shortcut: "7" },
];

export const SETTINGS_ROUTE: Route = {
  id: "settings",
  label: "Settings",
  icon: Settings,
  phase: 1,
  ready: true,
  shortcut: ",",
};
