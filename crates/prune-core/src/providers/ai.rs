//! What AI tools keep on disk: downloaded models, agent scratch space, conversation history.
//!
//! On a developer machine this is now often the largest thing there is, and none of it is in
//! the places a cleaner looks: it sits in dot-folders in the home directory, not in the
//! system's cache directory. It is also two different kinds of thing, and the risk levels keep
//! them apart:
//!
//! - **Downloaded models** come back if they are deleted: the next program that wants one
//!   downloads it again. That can take an hour and needs the network, so they are Medium — never
//!   selected automatically, by the interface or by `prune clean --yes`.
//! - **History and work** do not come back: a model's conversations, git worktrees an agent made
//!   (which can hold uncommitted changes), images it generated. They are Protected — listed with
//!   their size so a person can see where the space went, and never removable through Prune.
//!   Protected targets are also left out of everything called reclaimable.
//!
//! Locations are each tool's default. Tools that let the location be moved with an environment
//! variable (`HF_HOME`, `OLLAMA_MODELS`, `CODEX_HOME`) are only found where they default to.

use std::path::{Path, PathBuf};

use crate::models::{Category, RiskLevel};
use crate::providers::{Root, SimpleProvider};

const AI: Category = Category::AiData;

/// Folders under `~/.cache` that the generic `~/.cache` provider leaves alone: those another
/// provider already reports (here, or in tool_caches.rs), and a program's own runtime, which it
/// loads while it runs — removing it breaks the program rather than freeing a cache.
const NOT_GENERIC_IN_DOT_CACHE: &[&str] = &[
    // Reported by a provider in this file.
    "huggingface",
    "torch",
    "whisper",
    "lm-studio",
    // Reported in tool_caches.rs.
    "Cypress",
    "composer",
    "deno",
    "dotslash",
    "ms-playwright",
    "pip",
    "pnpm",
    "puppeteer",
    "uv",
    // Runtimes.
    "codex-runtimes",
];

fn whole_if(path: PathBuf) -> Vec<Root> {
    vec![Root::whole(path)]
}

/// Every directory one level down, each as a target: `~/.lmstudio/models/<publisher>/<model>`.
fn grandchildren(parent: &Path) -> Vec<Root> {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut roots: Vec<Root> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(Root::children)
        .collect();
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots
}

/// Files in `dir` whose names start with `prefix`: Codex's numbered history databases.
fn files_starting_with(dir: &Path, prefix: &str) -> Vec<Root> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut roots: Vec<Root> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        })
        .map(Root::whole)
        .collect();
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots
}

pub const HUGGING_FACE_MODELS: SimpleProvider = SimpleProvider::new(
    "ai_hugging_face",
    "Hugging Face models",
    AI,
    "Models and datasets downloaded from Hugging Face, one folder each. A program that needs one \
     again downloads it again — for a large model that can take an hour and needs the network.",
    RiskLevel::Medium,
    |k| vec![Root::children(k.home.join(".cache/huggingface/hub"))],
)
.exclude(&[
    ".locks",
    "version.txt",
    "version_diffusers_cache.txt",
    "blobs",
]);

pub const LM_STUDIO_MODELS: SimpleProvider = SimpleProvider::new(
    "ai_lm_studio",
    "LM Studio models",
    AI,
    "Models downloaded in LM Studio, one folder per model. LM Studio downloads a model again \
     when it is next chosen.",
    RiskLevel::Medium,
    |k| {
        let mut roots = grandchildren(&k.home.join(".lmstudio/models"));
        // Where versions before 0.3 kept them.
        roots.extend(grandchildren(&k.home.join(".cache/lm-studio/models")));
        roots
    },
);

pub const PYTORCH_HUB: SimpleProvider = SimpleProvider::new(
    "ai_pytorch_hub",
    "PyTorch hub",
    AI,
    "Models and code fetched with torch.hub. Downloaded again when next loaded.",
    RiskLevel::Medium,
    |k| whole_if(k.home.join(".cache/torch")),
);

pub const WHISPER_MODELS: SimpleProvider = SimpleProvider::new(
    "ai_whisper",
    "Whisper models",
    AI,
    "Speech-recognition models downloaded by OpenAI Whisper. Downloaded again when next used.",
    RiskLevel::Medium,
    |k| whole_if(k.home.join(".cache/whisper")),
);

pub const OLLAMA_MODELS: SimpleProvider = SimpleProvider::new(
    "ai_ollama",
    "Ollama models",
    AI,
    "Models Ollama has pulled. Shown for their size only: remove them with `ollama rm <name>`. \
     Several models can share the same files, and deleting the folder directly leaves Ollama's \
     index pointing at data that is gone.",
    RiskLevel::Protected,
    |k| whole_if(k.home.join(".ollama/models")),
);

pub const CODEX_HISTORY: SimpleProvider = SimpleProvider::new(
    "ai_codex_history",
    "Codex history and work",
    AI,
    "Conversations, generated images and git worktrees kept by Codex. Shown for their size only. \
     A worktree can hold uncommitted changes, so Prune never removes one: use `git worktree \
     remove` from its repository once its work is merged.",
    RiskLevel::Protected,
    |k| {
        let codex = k.home.join(".codex");
        let mut roots = vec![
            Root::labelled(codex.join("sessions"), "Conversations"),
            Root::labelled(codex.join("archived_sessions"), "Archived conversations"),
            Root::labelled(codex.join("generated_images"), "Generated images"),
            Root::children(codex.join("worktrees")),
        ];
        roots.extend(files_starting_with(&codex, "thread_history"));
        roots
    },
);

pub const CODEX_SCRATCH: SimpleProvider = SimpleProvider::new(
    "ai_codex_scratch",
    "Codex scratch folders",
    AI,
    "Temporary folders Codex made while working: builds, logs, test runs. Anything touched in the \
     last day is left out, since a task may still be using it. Check before removing one that \
     was kept as evidence of a release or a test.",
    RiskLevel::Medium,
    |k| vec![Root::children(k.home.join(".codex/tmp"))],
)
.min_age_days(1);

pub const CLAUDE_CODE_HISTORY: SimpleProvider = SimpleProvider::new(
    "ai_claude_code_history",
    "Claude Code history",
    AI,
    "Conversation transcripts Claude Code keeps per project. Shown for their size only; Prune \
     never removes them.",
    RiskLevel::Protected,
    |k| vec![Root::children(k.home.join(".claude/projects"))],
);

pub const CLAUDE_DESKTOP_VM: SimpleProvider = SimpleProvider::new(
    "ai_claude_desktop_vm",
    "Claude desktop virtual machine",
    AI,
    "The virtual machine image the Claude desktop app runs its agent features in. Shown for its \
     size only: the app manages it, and removing it while the app uses it would break them.",
    RiskLevel::Protected,
    |k| {
        k.app_support
            .as_ref()
            .map(|support| vec![Root::whole(support.join("Claude/vm_bundles"))])
            .unwrap_or_default()
    },
);

pub const DOT_CACHE: SimpleProvider = SimpleProvider::new(
    "ai_dot_cache",
    "Other folders in ~/.cache",
    AI,
    "Folders command-line and AI tools keep in ~/.cache. Prune does not know what is in them: \
     some are caches that are rebuilt, others hold downloaded models, virtual environments or a \
     project's own files, and a program may be using one right now. Look before removing.",
    RiskLevel::High,
    |k| {
        let dot_cache = k.home.join(".cache");
        // Where the system's cache directory is ~/.cache itself (Linux), it is already scanned
        // as application caches; listing it twice would count it twice.
        if k.user_cache.as_deref() == Some(dot_cache.as_path()) {
            return Vec::new();
        }
        vec![Root::children(dot_cache)]
    },
)
.exclude(NOT_GENERIC_IN_DOT_CACHE);

/// In display order: what can be downloaded again first, then what can only be looked at.
pub const ALL: &[&SimpleProvider] = &[
    &HUGGING_FACE_MODELS,
    &LM_STUDIO_MODELS,
    &PYTORCH_HUB,
    &WHISPER_MODELS,
    &CODEX_SCRATCH,
    &DOT_CACHE,
    &OLLAMA_MODELS,
    &CODEX_HISTORY,
    &CLAUDE_CODE_HISTORY,
    &CLAUDE_DESKTOP_VM,
];
