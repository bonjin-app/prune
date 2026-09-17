import { useEffect } from "react";
import { PageHeader } from "@/components/ui/PageHeader";
import { abbreviatePath } from "@/lib/format";
import { useSettings } from "@/stores/settings";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { DEVELOPER_CATEGORIES } from "@/types/models";
import { CleanerWorkspace } from "../cleaner/CleanerWorkspace";
import { DockerCard } from "./DockerCard";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";

export function DeveloperView() {
  const view = useSettings((s) => s.view);
  const load = useSettings((s) => s.load);
  const setUiView = useUi((s) => s.setView);
  const home = useSystem((s) => s.info?.homeDir);

  useEffect(() => {
    if (!view) void load();
  }, [view, load]);

  const roots = view?.effectiveProjectRoots ?? [];
  const shown = roots.slice(0, 3).map((r) => abbreviatePath(r, home));
  const rest = roots.length - shown.length;

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Developer"
        description={
          roots.length > 0 ? (
            <>
              Tool caches, plus project artifacts in{" "}
              <span className="font-mono text-[11.5px]">{shown.join(", ")}</span>
              {rest > 0 && ` and ${rest} more`}
              {!view?.projectRootsConfigured && " (guessed)"}.{" "}
              <button
                type="button"
                onClick={() => setUiView("settings")}
                className="underline underline-offset-2 hover:text-fg"
              >
                Change
              </button>
            </>
          ) : (
            "Package manager caches, build outputs and project artifacts your tools can regenerate."
          )
        }
        actions={<DeleteModeToggle />}
      />
      <div className="px-7 pb-3">
        <DockerCard />
      </div>
      <CleanerWorkspace
        scope="developer"
        categories={DEVELOPER_CATEGORIES}
        emptyTitle="Prune your toolchain"
        emptyDescription="node_modules, Rust target/, Gradle and Maven caches, Xcode DerivedData, pnpm, npm, Cargo, pip and more. Everything here is rebuilt by your tools on demand."
      />
    </div>
  );
}
