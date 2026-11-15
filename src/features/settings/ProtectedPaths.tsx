import { useEffect, useState } from "react";
import { AlertTriangle, File, Folder, Shield, ShieldPlus, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { Spinner } from "@/components/ui/Spinner";
import { abbreviatePath } from "@/lib/format";
import { useSettings } from "@/stores/settings";

/**
 * Folders and files Prune must never remove, by any route: cleanup, the storage explorer, or
 * the uninstaller. Anything that contains one is kept too, because removing a folder removes
 * everything in it.
 *
 * The backend enforces this; the list here only edits it. While the settings file cannot be
 * read, what is protected is unknown, so nothing is removed, and this card says why.
 */
export function ProtectedPaths() {
  const view = useSettings((s) => s.view);
  const load = useSettings((s) => s.load);
  const saving = useSettings((s) => s.saving);
  const add = useSettings((s) => s.addProtectedPath);
  const remove = useSettings((s) => s.removeProtectedPath);
  const pick = useSettings((s) => s.pickProtectedPath);
  const error = useSettings((s) => s.error);
  const [draft, setDraft] = useState("");

  // Read again each time the card is shown: `prune protect` may have changed the list since.
  useEffect(() => {
    void load();
  }, [load]);

  const home = view?.homeDir;
  const paths = view?.settings.protectedPaths ?? [];

  const submit = async () => {
    if (saving || !view) return;
    if (await add(draft)) setDraft("");
  };

  return (
    <Card>
      <CardHeader
        title="Protected folders and files"
        subtitle="Prune never removes these, or anything that contains them, whatever a scan says about them."
      />
      <div className="px-4 pb-4">
        {view?.problem && (
          <div
            role="alert"
            className="mb-3 flex gap-2 rounded-md border border-warn/30 bg-warn-soft/40 px-3 py-2 text-[12px]"
          >
            <AlertTriangle size={14} className="mt-0.5 shrink-0 text-warn" />
            <p className="min-w-0 break-words">{view.problem}</p>
          </div>
        )}
        <ul className="mb-2 flex flex-col gap-1" aria-label="Protected paths">
          {paths.map((path) => (
            <li
              key={path}
              className="flex items-center gap-2 rounded-md border border-line bg-surface-2/50 px-2.5 py-1.5"
            >
              <Shield size={12} className="shrink-0 text-fg-faint" />
              <span className="min-w-0 flex-1 truncate font-mono text-[11.5px]" title={path}>
                {abbreviatePath(path, home)}
              </span>
              <button
                type="button"
                onClick={() => void remove(path)}
                disabled={saving}
                className="rounded p-0.5 text-fg-faint hover:text-danger disabled:opacity-40"
                aria-label={`Stop protecting ${path}`}
              >
                <X size={13} />
              </button>
            </li>
          ))}
          {view && paths.length === 0 && (
            <li className="px-1 py-1 text-[12px] text-fg-faint">
              Nothing is protected yet. Add a folder you keep on purpose, such as build output a
              project commits or a cache you rely on.
            </li>
          )}
        </ul>

        <div className="flex items-center gap-2">
          <div className="field flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-surface px-2.5">
            <Shield size={13} className="shrink-0 text-fg-faint" />
            <input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void submit()}
              placeholder="~/Documents/keep or a full path"
              spellCheck={false}
              aria-label="Folder or file to protect"
              className="min-w-0 flex-1 bg-transparent font-mono text-[12px] outline-none placeholder:text-fg-faint"
            />
          </div>
          <Button
            size="md"
            onClick={() => void submit()}
            disabled={!draft.trim() || saving || !view}
          >
            {saving ? <Spinner size={12} /> : <ShieldPlus size={13} />} Protect
          </Button>
        </div>

        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void pick("folder")}
            disabled={saving || !view}
          >
            <Folder size={12} /> Choose a folder…
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void pick("file")}
            disabled={saving || !view}
          >
            <File size={12} /> Choose a file…
          </Button>
        </div>

        {error && <p className="mt-2 text-[12px] text-danger">{error}</p>}
        <p className="mt-2 text-[11.5px] text-fg-faint">
          A protected item still shows up in scans, marked as protected, so you can see what it
          holds. A path that is missing stays protected and is covered again when it comes back.
        </p>
      </div>
    </Card>
  );
}
