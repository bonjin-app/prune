import { useEffect, useState } from "react";
import { FolderPlus, Home, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { Spinner } from "@/components/ui/Spinner";
import { abbreviatePath } from "@/lib/format";
import { useSettings } from "@/stores/settings";

/**
 * Where the developer scan looks for `node_modules`, `target/` and friends.
 *
 * Until this is set Prune guesses from a list of common folder names, which both misses code
 * kept somewhere unusual and walks trees the user never wanted scanned.
 */
export function ProjectRoots() {
  const view = useSettings((s) => s.view);
  const load = useSettings((s) => s.load);
  const saving = useSettings((s) => s.saving);
  const add = useSettings((s) => s.addProjectRoot);
  const remove = useSettings((s) => s.removeProjectRoot);
  const error = useSettings((s) => s.error);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!view) void load();
  }, [view, load]);

  const home = view?.homeDir;
  const configured = view?.settings.projectRoots ?? [];
  const effective = view?.effectiveProjectRoots ?? [];

  const submit = async () => {
    if (await add(draft)) setDraft("");
  };

  return (
    <Card>
      <CardHeader
        title="Project folders"
        subtitle={
          view?.projectRootsConfigured
            ? "The developer scan searches these folders for project artifacts."
            : "Prune is guessing where your code lives. Add a folder to search exactly where you want."
        }
      />
      <div className="px-4 pb-4">
        <ul className="mb-2 flex flex-col gap-1">
          {(view?.projectRootsConfigured ? configured : effective).map((root) => (
            <li
              key={root}
              className="flex items-center gap-2 rounded-md border border-line bg-surface-2/50 px-2.5 py-1.5"
            >
              <span className="min-w-0 flex-1 truncate font-mono text-[11.5px]">
                {abbreviatePath(root, home)}
              </span>
              {view?.projectRootsConfigured ? (
                <button
                  type="button"
                  onClick={() => void remove(root)}
                  disabled={saving}
                  className="rounded p-0.5 text-fg-faint hover:text-danger disabled:opacity-40"
                  aria-label={`Remove ${root}`}
                >
                  <X size={13} />
                </button>
              ) : (
                <span className="text-[10.5px] text-fg-faint">guessed</span>
              )}
            </li>
          ))}
          {effective.length === 0 && (
            <li className="px-1 py-1 text-[12px] text-fg-faint">
              No project folders found. Add one below.
            </li>
          )}
        </ul>

        <div className="flex items-center gap-2">
          <div className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
            <Home size={13} className="shrink-0 text-fg-faint" />
            <input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void submit()}
              placeholder={home ? `${home}/Projects` : "Folder to search"}
              spellCheck={false}
              aria-label="Folder to add"
              className="min-w-0 flex-1 bg-transparent font-mono text-[12px] outline-none placeholder:text-fg-faint"
            />
          </div>
          <Button size="md" onClick={() => void submit()} disabled={!draft.trim() || saving}>
            {saving ? <Spinner size={12} /> : <FolderPlus size={13} />} Add
          </Button>
        </div>

        {error && <p className="mt-2 text-[12px] text-danger">{error}</p>}
        <p className="mt-2 text-[11.5px] text-fg-faint">
          Folders must be inside your home folder, because that is the only place Prune is allowed
          to remove anything.
        </p>
      </div>
    </Card>
  );
}
