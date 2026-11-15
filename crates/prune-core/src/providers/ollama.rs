//! Ollama's models, one row each, with what each one actually costs.
//!
//! Ollama keeps a model as a manifest naming content-addressed files ("blobs") that several
//! models can share: two tags of one model often share the weights. A single row for the whole
//! folder said where the space went but not which model to remove. So the manifests are read
//! here, and each blob's bytes are counted once:
//!
//! - **A model's own files**: blobs only it uses. Removing the model frees these.
//! - **Shared files**: blobs two or more models use, one row per set of models sharing them.
//!   They stay until the last of those models goes.
//! - **Files no model uses**: an interrupted or replaced download, say, as one row.
//!
//! The rows add up to the blobs on disk, so nothing is counted twice. Each row is anchored on a
//! real file (a manifest, or a blob), never on the blobs folder, which would contain the others. Everything here is
//! [`RiskLevel::Protected`]: removing files behind Ollama's back leaves its index pointing at
//! data that is gone, and `ollama rm` is the way to remove a model. The manifests are only read.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::{CleanupProvider, ScanContext, ScanOutput};
use crate::models::{Category, CleanupTarget, RiskLevel, TargetKind};
use crate::platform::KnownPaths;

pub const ID: &str = "ai_ollama";

/// Where Ollama keeps its models unless `OLLAMA_MODELS` moves them. A desktop app opened from
/// the Finder does not see shell variables, so only the default is looked at.
fn models_dir(known: &KnownPaths) -> PathBuf {
    known.home.join(".ollama/models")
}

pub struct OllamaModels;

/// One manifest: which model it is and which blobs it names.
#[derive(Debug)]
struct Manifest {
    path: PathBuf,
    label: String,
    digests: Vec<String>,
    modified: Option<DateTime<Utc>>,
}

impl CleanupProvider for OllamaModels {
    fn id(&self) -> &str {
        ID
    }

    fn name(&self) -> &str {
        "Ollama models"
    }

    fn category(&self) -> Category {
        Category::AiData
    }

    fn description(&self) -> &str {
        "Models Ollama has pulled, each with the space only it uses. Shown for their size only: \
         remove one with `ollama rm <name>`. Several models can share files, and deleting them \
         directly leaves Ollama's index pointing at data that is gone."
    }

    fn default_risk(&self) -> RiskLevel {
        RiskLevel::Protected
    }

    fn is_available(&self, known: &KnownPaths) -> bool {
        models_dir(known).exists()
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let mut out = ScanOutput::default();
        let root = models_dir(ctx.known);
        let blobs_dir = root.join("blobs");

        let manifests = read_manifests(&root.join("manifests"), &mut out);
        let blobs = read_blobs(&blobs_dir, &mut out);
        if ctx.is_cancelled() {
            return out;
        }

        let mut users: HashMap<&str, usize> = HashMap::new();
        for manifest in &manifests {
            let unique: HashSet<&str> = manifest.digests.iter().map(String::as_str).collect();
            for digest in unique {
                *users.entry(digest).or_default() += 1;
            }
        }

        for manifest in &manifests {
            let mut own = (0u64, 0u64);
            let mut shared = 0u64;
            let mut missing = 0usize;
            let unique: HashSet<&str> = manifest.digests.iter().map(String::as_str).collect();
            for digest in unique {
                match blobs.get(digest) {
                    Some(blob) if users.get(digest) == Some(&1) => {
                        own.0 += blob.size;
                        own.1 += 1;
                    }
                    Some(blob) => shared += blob.size,
                    None => missing += 1,
                }
            }
            let mut description = format!("Remove it with `ollama rm {}`.", manifest.label);
            if shared > 0 {
                description.push_str(&format!(
                    " It also uses {} it shares with other models, freed only when the last of \
                     them is removed.",
                    decimal_bytes(shared)
                ));
            }
            if missing > 0 {
                description.push_str(&format!(
                    " {missing} of its files are missing; `ollama pull` fetches them again."
                ));
            }
            out.targets.push(target(
                &manifest.path,
                manifest.label.clone(),
                own,
                description,
                manifest.modified,
                TargetKind::File,
            ));
            (ctx.progress)(own.1, own.0, &manifest.path);
        }

        // Shared blobs, one row per set of models sharing them: "used by qwen:7b and
        // qwen:7b-chat" says which removals would free it. Each row is anchored on a real file,
        // the largest blob in the set, so no row sits inside another's folder.
        let mut shared: HashMap<Vec<&str>, Vec<&Blob>> = HashMap::new();
        let mut orphans: Vec<&Blob> = Vec::new();
        for (digest, blob) in &blobs {
            match users.get(digest.as_str()) {
                Some(n) if *n > 1 => {
                    let mut names: Vec<&str> = manifests
                        .iter()
                        .filter(|m| m.digests.iter().any(|d| d == digest))
                        .map(|m| m.label.as_str())
                        .collect();
                    names.sort_unstable();
                    names.dedup();
                    shared.entry(names).or_default().push(blob);
                }
                Some(_) => {}
                None => orphans.push(blob),
            }
        }
        let mut shared: Vec<_> = shared.into_iter().collect();
        shared.sort_by(|a, b| a.0.cmp(&b.0));
        for (names, set) in shared {
            let Some(anchor) = set.iter().max_by_key(|b| (b.size, &b.path)) else {
                continue;
            };
            let bytes = set.iter().map(|b| b.size).sum();
            out.targets.push(target(
                &anchor.path,
                format!("Shared by {}", join_names(&names)),
                (bytes, set.len() as u64),
                "Model files these models have in common. They are freed only when every one \
                 of them has been removed with `ollama rm`."
                    .into(),
                anchor.modified,
                TargetKind::File,
            ));
        }
        // Files no manifest names, as one row: on a machine that has pulled a model split into
        // many tensors there can be hundreds of them.
        if let Some(anchor) = orphans.iter().max_by_key(|b| (b.size, &b.path)) {
            let bytes = orphans.iter().map(|b| b.size).sum();
            let count = orphans.len();
            out.targets.push(target(
                &anchor.path,
                if count == 1 {
                    "File no model uses".into()
                } else {
                    format!("{count} files no model uses")
                },
                (bytes, count as u64),
                "No model Ollama lists names these files. An interrupted or replaced download \
                 usually leaves them. Prune does not remove them, because during a pull Ollama \
                 writes files before the model that names them exists."
                    .into(),
                orphans.iter().filter_map(|b| b.modified).max(),
                TargetKind::File,
            ));
        }
        out
    }
}

fn target(
    path: &Path,
    label: String,
    (bytes, files): (u64, u64),
    description: String,
    modified: Option<DateTime<Utc>>,
    kind: TargetKind,
) -> CleanupTarget {
    CleanupTarget {
        id: CleanupTarget::make_id(ID, path),
        provider_id: ID.into(),
        path: path.to_string_lossy().into_owned(),
        kind,
        size_bytes: bytes,
        file_count: files,
        risk: RiskLevel::Protected,
        label,
        description: Some(description),
        modified_at: modified,
        permanent_only: false,
        group: None,
    }
}

#[derive(Debug)]
struct Blob {
    path: PathBuf,
    size: u64,
    modified: Option<DateTime<Utc>>,
}

/// Every blob, keyed by digest as manifests write it (`sha256:<hex>`). Ollama names the files
/// `sha256-<hex>`; versions before 0.1.30 used the digest itself.
fn read_blobs(dir: &Path, out: &mut ScanOutput) -> HashMap<String, Blob> {
    let mut blobs = HashMap::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return blobs,
        Err(e) => {
            out.issue(dir, e.to_string());
            return blobs;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let digest = name.replacen('-', ":", 1);
        blobs.insert(
            digest,
            Blob {
                size: meta.len(),
                modified: meta.modified().ok().map(DateTime::<Utc>::from),
                path,
            },
        );
    }
    blobs
}

/// Manifests sit at `manifests/<registry>/<namespace>/<model>/<tag>`.
fn read_manifests(dir: &Path, out: &mut ScanOutput) -> Vec<Manifest> {
    let mut found = Vec::new();
    walk(dir, dir, 0, &mut found, out);
    found.sort_by(|a, b| a.label.cmp(&b.label));
    found
}

fn walk(base: &Path, dir: &Path, depth: usize, found: &mut Vec<Manifest>, out: &mut ScanOutput) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() && depth < 3 {
            walk(base, &path, depth + 1, found, out);
        } else if meta.is_file() && depth == 3 {
            match parse(base, &path) {
                Some(mut manifest) => {
                    manifest.modified = meta.modified().ok().map(DateTime::<Utc>::from);
                    found.push(manifest);
                }
                None => out.issue(&path, "not a manifest Prune can read"),
            }
        }
    }
}

fn parse(base: &Path, path: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut digests = Vec::new();
    let mut take = |v: &serde_json::Value| {
        if let Some(d) = v.get("digest").and_then(|d| d.as_str()) {
            digests.push(d.to_string());
        }
    };
    if let Some(config) = json.get("config") {
        take(config);
    }
    for layer in json.get("layers")?.as_array()? {
        take(layer);
    }
    let parts: Vec<String> = path
        .strip_prefix(base)
        .ok()?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let [registry, namespace, model, tag] = parts.as_slice() else {
        return None;
    };
    Some(Manifest {
        path: path.to_path_buf(),
        label: model_name(registry, namespace, model, tag),
        digests,
        modified: None,
    })
}

/// `a`, `a and b`, `a, b and c`.
fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Decimal units, as the app and Finder show sizes, for the one size written into a sentence.
fn decimal_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The name `ollama list` shows, which is also what `ollama rm` takes.
fn model_name(registry: &str, namespace: &str, model: &str, tag: &str) -> String {
    match (registry, namespace) {
        ("registry.ollama.ai", "library") => format!("{model}:{tag}"),
        ("registry.ollama.ai", _) => format!("{namespace}/{model}:{tag}"),
        _ => format!("{registry}/{namespace}/{model}:{tag}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_model_names_as_a_sentence() {
        assert_eq!(join_names(&["a"]), "a");
        assert_eq!(join_names(&["a", "b"]), "a and b");
        assert_eq!(join_names(&["a", "b", "c"]), "a, b and c");
    }

    #[test]
    fn writes_sizes_in_decimal_units() {
        assert_eq!(decimal_bytes(512), "512 B");
        assert_eq!(decimal_bytes(1_929_903_008), "1.9 GB");
    }

    #[test]
    fn names_models_the_way_ollama_lists_them() {
        assert_eq!(
            model_name("registry.ollama.ai", "library", "qwen2.5", "latest"),
            "qwen2.5:latest"
        );
        assert_eq!(
            model_name("registry.ollama.ai", "huihui_ai", "qwen3", "8b"),
            "huihui_ai/qwen3:8b"
        );
        assert_eq!(
            model_name("hf.co", "org", "model-GGUF", "Q4_K_M"),
            "hf.co/org/model-GGUF:Q4_K_M"
        );
    }
}
