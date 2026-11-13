# Prune GUI quality goals

Approved scope: 2026-10-08. Independently implemented MIT desktop GUI inspired by Mole's
workflows, not a port of its GPL CLI. See [feature mapping](MOLE-GUI.md).

## Product target clarified on 2026-10-15

User requirement: Prune must reach Mole-class open-source product quality, not merely
add a few cleanup rules. This is a quality target, not a claim of current parity or
approval to bypass the pending architecture/design reviews.

The comparison has two distinct references, checked on 2026-10-15:

- [Mole's open-source CLI](https://github.com/tw93/Mole): cleaning, app removal,
  disk analysis, project artifacts, installers and monitoring, with explicit safety rules.
- [Mole's separate native app](https://mole.fit/): a GUI reference for review workflows,
  explanations, drill-down and installation experience. Its app is not the open-source
  CLI. Reference features are not permission to copy its code, branding or artwork.

Prune remains an independently implemented MIT GUI and CLI. It must not require Mole
to be installed or invoke Mole to perform its work. No performance comparison is
established by reading either product's documentation.

### Completion criteria

| Dimension                 | Evidence required before claiming the target achieved                                                                                                                                                                                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Useful discovery          | A synthetic RiderLink-shaped repository with root/app-local, dated Xcode build directories is explained by project, artifact type and modification time. Source, credentials, signing data, archives, dSYMs and QA evidence remain protected. Recognizing a large folder alone is not cleanup eligibility.                |
| Native safety             | Git-tracked files, nested repositories and user-protected paths are enforced in discovery, preview and execution. Unreadable/unknown protection state fails closed. Fresh process-use evidence and unknown activity are represented explicitly before any new activity-sensitive cleanup capability is enabled.           |
| Truthful accounting       | Found logical bytes, reviewable bytes, blocked/unknown bytes, bytes moved to Trash and current OS available capacity are distinct. Never present a Trash move or an allocated-size sum as measured reclaimed physical storage.                                                                                            |
| Large-scan behavior       | Record first visible progress, completion time, peak memory and cancellation acknowledgement/settlement on named fixtures and a user-approved read-only workload. Distinguish cold/warm runs, exclusions, permission waits and partial results. A bounded targeted refresh must not silently trigger a whole-home rescan. |
| Scan performance claims   | Use the same fixture, machine, filesystem coverage and measurement method across runs; retain at least three repeat measurements. Do not claim faster-than-Mole without an authorized, comparable benchmark. Saved observations, if later introduced, never grant cleanup authority.                                      |
| GUI quality               | Keyboard/focus, light/dark themes, compact windows, pagination, empty/error/partial states and duplicate-action protection pass automated and native fixture checks. Repository activity and artifact modification are labelled separately; age is not proof of inactivity.                                               |
| Recoverability and impact | Preview lists exact targets, blocked reasons, irreversible operations and likely rebuild/download cost. Trash stays the default when supported; explain OS recovery and that storage remains occupied in Trash. An in-app restore feature is not implied.                                                                 |
| Open-source distribution  | Reproducible build instructions, CI, tests, security reporting, license and release notes match the shipped artifact. Verify native app/installer integrity and source provenance; record signing, notarization and actual platform verification separately. A local ad-hoc build is not a public production release.     |

Existing features such as app removal, installer review and monitoring also need their
own current verification; a successful disk-cleaning milestone does not certify them.
AI model/session inventory, app updating, privileged maintenance, menu-bar tools and
automatic schedules remain separately designed capabilities, not hidden scope in the
first milestone. Missing features must stay visible in the comparison matrix rather
than being described as full feature parity.

### First milestone and review boundary

The first milestone is the existing
[storage-foundation design](superpowers/specs/2026-10-11-storage-foundation-design.md):
backend artifact guards, user protections, truthful review totals and folder-explorer
reliability. That written design is still awaiting user review; no implementation plan
exists yet. Expanding destructive Xcode rules before those protections is not the first
milestone.

Next, design project-local Xcode inventory and narrowly bounded artifact review using
RiderLink-shaped fixtures, then address large-scan performance and read-only AI inventory
as separate milestones. Never label an entire `.local` folder disposable; it may mix
compilation output, symbols, test evidence, logs and credentials.

These requirements do not authorize real-user deletion, Mole installation, dependency
installation, global settings changes, app replacement or release publication. Preserve
the existing uncommitted work and report implementation, automated tests, native evidence,
installed artifacts and publication as separate statuses.

## This implementation pass

| Goal                          | Acceptance evidence                                                                                                                                                                                                        |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Honest storage accounting     | Logical sizes are labelled separately from current OS available capacity. No claim that a Trash move freed its reported bytes. Capacity-read failure must not change a cleanup result.                                     |
| Explicit cancellation         | Both analyzers acknowledge a cancel request immediately, queue requests made before launch returns, reject duplicate requests, and wait for completion before showing stopped. OS access can delay cancellation.           |
| Reliable asynchronous results | Bounded early-event buffering, matching scan IDs, stale-response guards and one-second completion reconciliation. Old preview or cleanup replies cannot replace a newer scan.                                              |
| Fail-closed cleanup feedback  | Only consistent confirmed results prune rows. Failed or inconsistent results retain rows and request a rescan; duplicate execution is rejected. Existing protected/source/session data and preview rules remain unchanged. |
| Bounded folder rendering      | At most 100 list rows per page, Previous/Next and range/total controls, whole-folder search, navigation/search reset and shrink clamping. A 1,001-folder regression verifies all pages and focus.                          |
| Reviewable removal impact     | High-risk items are not described as medium risk; blocked items are excluded from planned totals; rebuild/download cost and irreversible removal are explicit. No new automatic deletion policy.                           |
| Installable local artifact    | Apple Silicon app and DMG built from this tree, signature and image integrity checked, read-only fixture QA in the installed app. Older installed builds preserved as temporary backups.                                   |

Test counts, artifact hashes and observed native behavior belong in
[the verification record](VERIFICATION-0.2.0.md), not in promises based on implementation alone.

Cleanup consistency includes required fields, target/path identity, mode/status and logical
totals matching the plan. Active cleanup cannot be abandoned through rescan or dismissal.

## Before a public production release

- Developer ID signing, notarization and downloaded-app Gatekeeper validation.
- User-managed access for protected folders; never silently grant Full Disk Access. A stalled
  protected-folder scan is not evidence of a complete inventory.
- Actual Intel/Windows build and device evidence before claiming those artifacts verified.
- Real Unity re-import validation before claiming a measured rebuild cost.
- Explicit approval for publication, tags, releases or destructive user-data operations.

## Separate future design scope

Dedicated AI cache/session/model inventory, arbitrary-folder removal, privileged maintenance,
automatic schedules and expanded cleanup rules need their own impact/protection design and
approval. Large or old data is not automatically disposable. These are not hidden prerequisites
for this bounded stability pass and are not implemented by it.
