use serde::{Deserialize, Serialize};

/// How dangerous it is to remove something.
///
/// Ordered from safest to most dangerous so it can be compared with `<`/`>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Regenerated automatically, no user data. Browser caches, npm cache, logs.
    Safe,
    /// Cheap to rebuild but takes time. `node_modules`, build directories.
    Low,
    /// May contain state the user cares about. Application data, Xcode archives.
    Medium,
    /// Removal is likely to break something. Shown but never pre-selected.
    High,
    /// Never removable through Prune.
    Protected,
}

impl RiskLevel {
    /// Whether the UI may pre-select targets of this level in a "recommended" cleanup.
    pub fn preselect(self) -> bool {
        matches!(self, RiskLevel::Safe)
    }

    pub fn is_removable(self) -> bool {
        self != RiskLevel::Protected
    }
}

/// High-level grouping used by the Cleaner UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    SystemCache,
    ApplicationCache,
    BrowserCache,
    Logs,
    TemporaryFiles,
    Trash,
    DeveloperFiles,
    OldInstallers,
    LargeFiles,
    Applications,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::SystemCache => "System Cache",
            Category::ApplicationCache => "Application Cache",
            Category::BrowserCache => "Browser Cache",
            Category::Logs => "Logs",
            Category::TemporaryFiles => "Temporary Files",
            Category::Trash => "Trash",
            Category::DeveloperFiles => "Developer Files",
            Category::OldInstallers => "Old Installers",
            Category::LargeFiles => "Large Files",
            Category::Applications => "Applications",
        }
    }
}

/// How a target gets removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    /// Move to the OS trash / recycle bin (recoverable). Default.
    #[default]
    Trash,
    /// Remove immediately. Not recoverable.
    Permanent,
}
