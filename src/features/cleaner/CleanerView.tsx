import { PageHeader } from "@/components/ui/PageHeader";
import { cleanerSources } from "@/lib/wording";
import { useSystem } from "@/stores/system";
import { CLEANER_CATEGORIES } from "@/types/models";
import { CleanerWorkspace } from "./CleanerWorkspace";
import { DeleteModeToggle } from "./DeleteModeToggle";

export function CleanerView() {
  const platform = useSystem((s) => s.meta?.platform);
  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Cleaner"
        description="Caches, logs, temporary files and leftovers that are safe to remove."
        actions={<DeleteModeToggle />}
      />
      <CleanerWorkspace
        scope="cleaner"
        categories={CLEANER_CATEGORIES}
        emptyTitle="Find what can go"
        emptyDescription={`Prune looks through ${cleanerSources(platform)}. Nothing is removed without your review.`}
      />
    </div>
  );
}
