import { HardDrive } from "lucide-react";
import { PlaceholderView } from "@/components/ui/Placeholder";

export function DiskView() {
  return (
    <PlaceholderView
      icon={HardDrive}
      title="Disk"
      phase={5}
      description="Visual disk usage, directory tree and large file finder."
      bullets={[
        "Per-volume breakdown: Applications, Users, System, Developer, Other",
        "Directory tree with sizes and cancellable scans",
        "Large file finder with > 1 GB / 5 GB / 10 GB filters",
      ]}
    />
  );
}
