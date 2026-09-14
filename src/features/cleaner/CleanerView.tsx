import { PageHeader } from "@/components/ui/PageHeader";
import { CLEANER_CATEGORIES } from "@/types/models";
import { CleanerWorkspace } from "./CleanerWorkspace";
import { DeleteModeToggle } from "./DeleteModeToggle";

export function CleanerView() {
  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Cleaner"
        description="Caches, logs, temporary files and leftovers that are safe to remove."
        actions={<DeleteModeToggle />}
      />
      <CleanerWorkspace
        categories={CLEANER_CATEGORIES}
        emptyTitle="Find what can go"
        emptyDescription="Prune looks through application caches, logs, temporary files, browser caches, the Trash and old installers. Nothing is removed without your review."
      />
    </div>
  );
}
