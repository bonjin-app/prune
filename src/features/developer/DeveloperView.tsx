import { PageHeader } from "@/components/ui/PageHeader";
import { DEVELOPER_CATEGORIES } from "@/types/models";
import { CleanerWorkspace } from "../cleaner/CleanerWorkspace";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";

export function DeveloperView() {
  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Developer"
        description="Package manager caches, build outputs and project artifacts your tools can regenerate."
        actions={<DeleteModeToggle />}
      />
      <CleanerWorkspace
        categories={DEVELOPER_CATEGORIES}
        emptyTitle="Prune your toolchain"
        emptyDescription="node_modules, Rust target/, Gradle and Maven caches, Xcode DerivedData, pnpm, npm, Cargo, pip and more. Everything here is rebuilt by your tools on demand."
      />
    </div>
  );
}
