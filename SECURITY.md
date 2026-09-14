# Security Policy

Prune deletes files. We treat every bug that can cause unintended removal as a security issue.

## Supported Versions

Only the latest release receives fixes.

## Reporting a Vulnerability

Please **do not** open a public issue for problems that could lead to data loss or privilege
escalation. Use GitHub's private vulnerability reporting
(`Security` → `Report a vulnerability`) on `bonjin-app/prune`. You will get an acknowledgement
within 72 hours.

Include:

- OS and version
- Prune version (`Settings → About`)
- Steps to reproduce, ideally against a temporary directory
- Whether real files were affected

## Design Guarantees

These properties are enforced in `crates/prune-core` and covered by tests. A change that
weakens any of them requires a security review.

1. **Whitelist, not blacklist.** Removal is only possible inside the user's home directory and
   the system temp directory. Everything else is refused before any I/O happens.
2. **Protected paths.** System directories, the home directory itself, `~/Library`,
   `~/Documents`, `~/Desktop`, `~/.ssh`, keychains, mail, photos, iCloud and Windows
   equivalents are never removable, even when inside the whitelist.
3. **No raw paths over IPC for deletion.** The frontend sends _target ids_ from a scan and a
   _plan id_ from a preview. Paths are resolved and re-validated inside the engine right before
   removal.
4. **Dry run first.** A `CleanupPlan` is always produced and shown before anything is removed.
5. **Trash by default.** Permanent deletion is opt-in and visually distinct.
6. **Local only.** Prune has no network code, no telemetry, and no accounts. The operation log
   never leaves the machine.
