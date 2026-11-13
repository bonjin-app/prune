import { useEffect, useState } from "react";
import { PageHeader } from "@/components/ui/PageHeader";
import { abbreviatePath } from "@/lib/format";
import { useSettings } from "@/stores/settings";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";
import { DEVELOPER_CATEGORIES } from "@/types/models";
import { CleanerWorkspace } from "../cleaner/CleanerWorkspace";
import { DockerCard } from "./DockerCard";
import { DeleteModeToggle } from "../cleaner/DeleteModeToggle";
import { ProjectRoots } from "../settings/ProjectRoots";

function ProjectScanFolders({ suggestedPath }: { suggestedPath: string | null }) {
  const [open, setOpen] = useState(Boolean(suggestedPath));
  return (
    <details open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="cursor-pointer py-2 text-[12px] text-fg-muted">
        Project scan folders
      </summary>
      <ProjectRoots suggestedPath={suggestedPath} />
    </details>
  );
}

export function DeveloperView() {
  const view = useSettings((s) => s.view);
  const load = useSettings((s) => s.load);
  const setUiView = useUi((s) => s.setView);
  const suggestedPath = useUi((s) => s.projectRootDraft);
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
              Tool caches, AI models and data, and project artifacts in{" "}
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
            "Package manager caches, build outputs, project artifacts, and what AI tools keep on disk."
          )
        }
        actions={<DeleteModeToggle />}
      />
      <div className="px-7 pb-3">
        <ProjectScanFolders key={suggestedPath ?? ""} suggestedPath={suggestedPath} />
        <DockerCard />
      </div>
      <CleanerWorkspace
        scope="developer"
        categories={DEVELOPER_CATEGORIES}
        emptyTitle="Prune your toolchain"
        emptyDescription="node_modules, Rust target/, Gradle and Maven caches, Xcode DerivedData, pnpm, npm, Cargo, pip, and the models Hugging Face, LM Studio and Ollama download. Conversation history and agent worktrees are shown for their size only, never offered for removal."
      />
    </div>
  );
}
