//! Browser caches. Only *cache* directories — never cookies, history, passwords or sessions.

use std::path::{Path, PathBuf};

use super::{CleanupProvider, ScanContext, ScanOutput};
use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;

struct ChromiumBrowser {
    name: &'static str,
    /// Relative to `~/Library/Caches` (macOS) — holds `<Profile>/Cache`.
    mac_cache: &'static str,
    /// Relative to `~/Library/Application Support` (macOS) — holds `<Profile>/Code Cache`.
    mac_data: &'static str,
    /// Relative to `%LOCALAPPDATA%` (Windows) — `User Data/<Profile>/{Cache,Code Cache}`.
    win_data: &'static str,
}

const CHROMIUM: &[ChromiumBrowser] = &[
    ChromiumBrowser {
        name: "Google Chrome",
        mac_cache: "Google/Chrome",
        mac_data: "Google/Chrome",
        win_data: "Google\\Chrome\\User Data",
    },
    ChromiumBrowser {
        name: "Chromium",
        mac_cache: "Chromium",
        mac_data: "Chromium",
        win_data: "Chromium\\User Data",
    },
    ChromiumBrowser {
        name: "Brave",
        mac_cache: "BraveSoftware/Brave-Browser",
        mac_data: "BraveSoftware/Brave-Browser",
        win_data: "BraveSoftware\\Brave-Browser\\User Data",
    },
    ChromiumBrowser {
        name: "Microsoft Edge",
        mac_cache: "Microsoft Edge",
        mac_data: "Microsoft Edge",
        win_data: "Microsoft\\Edge\\User Data",
    },
    ChromiumBrowser {
        name: "Arc",
        mac_cache: "company.thebrowser.Browser",
        mac_data: "Arc/User Data",
        win_data:
            "Packages\\TheBrowserCompany.Arc_ttt1ap7aakyb4\\LocalCache\\Local\\Arc\\User Data",
    },
    ChromiumBrowser {
        name: "Vivaldi",
        mac_cache: "com.vivaldi.Vivaldi",
        mac_data: "Vivaldi",
        win_data: "Vivaldi\\User Data",
    },
    ChromiumBrowser {
        name: "Opera",
        mac_cache: "com.operasoftware.Opera",
        mac_data: "com.operasoftware.Opera",
        win_data: "Opera Software\\Opera Stable",
    },
];

const CACHE_DIRS: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnCache",
    "GrShaderCache",
    "ShaderCache",
];

fn is_profile_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    path.is_dir() && (name == "Default" || name.starts_with("Profile ") || name == "Guest Profile")
}

fn profiles(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };
    let mut v: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_profile_dir(p))
        .collect();
    v.sort();
    v
}

pub struct ChromiumCache;

impl ChromiumCache {
    fn roots(known: &KnownPaths) -> Vec<(String, PathBuf)> {
        let mut roots = Vec::new();
        for b in CHROMIUM {
            let mut profile_roots: Vec<PathBuf> = Vec::new();
            if cfg!(target_os = "windows") {
                if let Some(local) = &known.local_app_data {
                    profile_roots.push(local.join(b.win_data));
                }
            } else {
                if let Some(c) = &known.user_cache {
                    profile_roots.push(c.join(b.mac_cache));
                }
                if let Some(d) = &known.app_support {
                    profile_roots.push(d.join(b.mac_data));
                }
            }
            for root in profile_roots.into_iter().filter(|p| p.is_dir()) {
                for profile in profiles(&root) {
                    let profile_name = profile.file_name().unwrap().to_string_lossy().into_owned();
                    for dir in CACHE_DIRS {
                        let p = profile.join(dir);
                        if p.is_dir() {
                            roots.push((format!("{} · {} · {}", b.name, profile_name, dir), p));
                        }
                    }
                }
            }
        }
        roots
    }
}

impl CleanupProvider for ChromiumCache {
    fn id(&self) -> &str {
        "chromium_cache"
    }
    fn name(&self) -> &str {
        "Chromium Browsers"
    }
    fn category(&self) -> Category {
        Category::BrowserCache
    }
    fn description(&self) -> &str {
        "HTTP and code caches of Chrome, Brave, Edge, Arc, Vivaldi, Opera. Logins are untouched."
    }
    fn default_risk(&self) -> RiskLevel {
        RiskLevel::Safe
    }
    fn priority(&self) -> u8 {
        70
    }
    fn is_available(&self, known: &KnownPaths) -> bool {
        !Self::roots(known).is_empty()
    }
    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let mut out = ScanOutput::default();
        for (label, path) in Self::roots(ctx.known) {
            if ctx.is_cancelled() {
                break;
            }
            out.push_measured(ctx, self.id(), &path, label, RiskLevel::Safe, None, false);
        }
        out
    }
}

pub struct FirefoxCache;

impl FirefoxCache {
    fn roots(known: &KnownPaths) -> Vec<(String, PathBuf)> {
        let profiles_root = if cfg!(target_os = "windows") {
            known
                .local_app_data
                .as_ref()
                .map(|l| l.join("Mozilla\\Firefox\\Profiles"))
        } else {
            known
                .user_cache
                .as_ref()
                .map(|c| c.join("Firefox/Profiles"))
        };
        let Some(root) = profiles_root.filter(|p| p.is_dir()) else {
            return vec![];
        };
        let Ok(entries) = std::fs::read_dir(&root) else {
            return vec![];
        };
        let mut v: Vec<(String, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter_map(|p| {
                let cache = p.join("cache2");
                cache.is_dir().then(|| {
                    (
                        format!("Firefox · {}", p.file_name().unwrap().to_string_lossy()),
                        cache,
                    )
                })
            })
            .collect();
        v.sort();
        v
    }
}

impl CleanupProvider for FirefoxCache {
    fn id(&self) -> &str {
        "firefox_cache"
    }
    fn name(&self) -> &str {
        "Firefox"
    }
    fn category(&self) -> Category {
        Category::BrowserCache
    }
    fn description(&self) -> &str {
        "Firefox HTTP cache per profile. Bookmarks, history and logins are untouched."
    }
    fn default_risk(&self) -> RiskLevel {
        RiskLevel::Safe
    }
    fn priority(&self) -> u8 {
        70
    }
    fn is_available(&self, known: &KnownPaths) -> bool {
        !Self::roots(known).is_empty()
    }
    fn scan(&self, ctx: &ScanContext<'_>) -> ScanOutput {
        let mut out = ScanOutput::default();
        for (label, path) in Self::roots(ctx.known) {
            if ctx.is_cancelled() {
                break;
            }
            out.push_measured(ctx, self.id(), &path, label, RiskLevel::Safe, None, false);
        }
        out
    }
}
