# Mole-inspired GUI work

Reference inspected on 2026-10-08: [Mole CLI](https://github.com/tw93/mole),
[official Mac app](https://mole.fit/) and its Analyze / AI screenshots.
The CLI is GPL-3.0; the native app is a separate product. Prune remains an independently
implemented MIT project. No Mole source, branding, artwork or screenshots are bundled.

## GUI feature mapping

| Workflow      | Prune implementation                                                                                                                   | Remaining scope                                                                    |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Analyze       | Live capacity, whole-disk/external shortcuts, treemap, folder ranking, breadcrumbs, search, Finder reveal, explicit incomplete results | Context-menu review for arbitrary folders; scan cache persistence                  |
| Clean         | Categorized providers, risk levels, selection, mandatory preview, Trash/permanent modes, result and operation log                      | More application-specific leftovers and protected-item preferences                 |
| Project purge | Grouped build artifacts, activity hints, configured roots, Unity Library detection, Storage-to-Developer path suggestion               | Git-tracked/nested repository checks before expanding automatic cleanup rules      |
| AI data       | Analyze folders/worktrees and review rebuildable project artifacts without deleting sessions                                           | Dedicated cache/session/model inventory with explicit impact explanations          |
| Apps          | Installed-app sizing, linked leftovers, shared-data protection, vendor uninstall workflow                                              | Application update-source integration                                              |
| Status        | CPU/memory/disks/network/process monitoring and startup controls                                                                       | GPU, thermal, battery/accessory metrics and menu bar companion                     |
| Optimize      | No privileged maintenance runner                                                                                                       | Explicit task plans, app-busy checks, authorization, skip reasons and verification |

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
