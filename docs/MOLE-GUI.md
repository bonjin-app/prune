# Mole-inspired GUI work

Reference inspected on 2026-10-08: [Mole CLI](https://github.com/tw93/mole),
[official Mac app](https://mole.fit/) and its Analyze / AI screenshots.
The CLI is GPL-3.0; the native app is a separate product. Prune remains an independently
implemented MIT project. No Mole source, branding, artwork or screenshots are bundled.

## GUI feature mapping

| Workflow      | Prune implementation                                                                                                                                                                                                              | Remaining scope                                                                                                       |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Analyze       | Live capacity, whole-disk/external shortcuts, treemap, folder ranking, breadcrumbs, search, Finder reveal, explicit incomplete results, last finished analysis per folder remembered (read-only)                                  | Context-menu review for arbitrary folders                                                                             |
| Clean         | Categorized providers, risk levels, selection, mandatory preview, Trash/permanent modes, result and operation log, user-protected paths (Settings, `prune protect`, native picker) enforced at scan, preview and removal          | More application-specific leftovers                                                                                   |
| Project purge | Grouped build artifacts, activity hints, configured roots, Unity Library detection, Storage-to-Developer path suggestion, Git-tracked and nested-repository checks at scan, preview and removal                                   | Project-local Xcode inventory and other new rules, behind those checks                                                |
| AI data       | Read-only inventory of Hugging Face, LM Studio, PyTorch hub, Whisper, Ollama (per model, shared and unused files counted once), Codex and Claude Code history/worktrees, Claude VM; downloadable models Medium, history Protected | Locations moved by `HF_HOME`/`OLLAMA_MODELS`, ComfyUI/Stable Diffusion model folders, Gemini CLI                      |
| Apps          | Installed-app sizing, linked leftovers, shared-data protection, vendor uninstall workflow                                                                                                                                         | Update-source integration needs the network, which Prune never uses; out of scope unless that principle changes       |
| Status        | CPU/memory/disks/network/process monitoring, startup controls, battery charge/wear/cycles, processor/battery/storage temperatures and GPU load (CLI too), and a menu bar icon with the same figures                                                                                    | Accessory batteries                                                                                                  |
| Optimize      | No privileged maintenance runner                                                                                                                                                                                                  | Explicit task plans, app-busy checks, authorization, skip reasons and verification; needs its own design and approval |

## Product rules

- Keep a large folder's next step nearby: drill down, reveal, or review rebuildable artifacts.
- Keep file-size measurements separate from live allocated capacity. Partial is not zero.
- A handoff suggests a scan folder. Only **Add** changes scan settings; only **Scan** reads
  cleanup targets; only preview followed by confirmation removes selected targets.
- Preserve existing scan roots when adding a suggested path. Never choose source or session
  deletion just because an item is large or old.
- No terminal commands, network access or Mole installation are required to use the GUI.
- Review new high-risk capabilities separately. Do not disguise OS service resets as storage
  cleanup, or claim a notarized/public release from a local ad-hoc build.
