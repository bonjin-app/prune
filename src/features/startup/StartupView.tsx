import { Power } from "lucide-react";
import { PlaceholderView } from "@/components/ui/Placeholder";

export function StartupView() {
  return (
    <PlaceholderView
      icon={Power}
      title="Startup"
      phase={7}
      description="Programs that launch when you log in."
      bullets={[
        "Login items, launch agents and registry Run keys",
        "Enable / disable without deleting",
        "Platform-specific handling in the Rust platform layer",
      ]}
    />
  );
}
