import { PackageMinus } from "lucide-react";
import { PlaceholderView } from "@/components/ui/Placeholder";

export function UninstallerView() {
  return (
    <PlaceholderView
      icon={PackageMinus}
      title="Uninstaller"
      phase={6}
      description="Remove applications together with their caches, preferences and containers."
      bullets={[
        "Installed application list with sizes",
        "Related data: caches, preferences, logs, containers, extensions",
        "Safe uninstall through the same preview → confirm → log pipeline",
      ]}
    />
  );
}
