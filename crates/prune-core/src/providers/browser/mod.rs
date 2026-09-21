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

/// Where a browser's profiles live, on the platform asked about.
///
/// Taking the platform as an argument rather than reading `cfg!` makes both answers reachable
/// from a test on either machine. The directories this decides are the ones the scan then looks
/// inside, so getting them wrong is how a cleanup ends up somewhere it was never meant to be.
fn profile_roots(b: &ChromiumBrowser, known: &KnownPaths, windows: bool) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if windows {
        if let Some(local) = &known.local_app_data {
            roots.push(local.join(b.win_data));
        }
    } else {
        if let Some(c) = &known.user_cache {
            roots.push(c.join(b.mac_cache));
        }
        if let Some(d) = &known.app_support {
            roots.push(d.join(b.mac_data));
        }
    }
    roots
}

impl ChromiumCache {
    fn roots(known: &KnownPaths) -> Vec<(String, PathBuf)> {
        let mut roots = Vec::new();
        for b in CHROMIUM {
            let profile_roots = profile_roots(b, known, cfg!(target_os = "windows"));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// What a real Chrome profile holds besides its caches. Nothing on this list may ever be
    /// reported: these are logins, sessions, site data and the user's own bookmarks and
    /// history. `CACHE_DIRS` is the only thing standing between them and a cleanup, so the
    /// point of these tests is to keep that list honest rather than to exercise the walk.
    const MUST_SURVIVE: &[&str] = &[
        "Cookies",
        "Login Data",
        "Web Data",
        "History",
        "Bookmarks",
        "Local Storage",
        "Session Storage",
        "IndexedDB",
        "Service Worker",
        "Extensions",
        "Local Extension Settings",
        "Preferences",
        "Affiliation Database",
        "ClientCertificates",
        "Accounts",
    ];

    fn known_for(dir: &std::path::Path) -> KnownPaths {
        KnownPaths {
            user_cache: Some(dir.join("Library/Caches")),
            app_support: Some(dir.join("Library/Application Support")),
            local_app_data: Some(dir.join("AppData/Local")),
            temp: dir.join("tmp"),
            home: dir.to_path_buf(),
            ..Default::default()
        }
    }

    /// Where `ChromiumCache::roots` will actually look on the machine running the test.
    ///
    /// `roots` reads `cfg!` to choose, so a fixture written to the macOS location finds nothing
    /// on Windows — which is the mistake these very tests exist to catch elsewhere.
    fn chrome_profile(known: &KnownPaths) -> std::path::PathBuf {
        profile_roots(&CHROMIUM[0], known, cfg!(target_os = "windows"))
            .into_iter()
            .next()
            .expect("a profile root for this platform")
            .join("Default")
    }

    /// The same question for Firefox.
    fn firefox_profile(known: &KnownPaths) -> std::path::PathBuf {
        let root = if cfg!(target_os = "windows") {
            known
                .local_app_data
                .as_ref()
                .unwrap()
                .join("Mozilla\\Firefox\\Profiles")
        } else {
            known.user_cache.as_ref().unwrap().join("Firefox/Profiles")
        };
        root.join("abc.default-release")
    }

    /// A Chrome profile as it really looks: caches beside everything that must not be touched.
    fn lay_out_profile(profile: &std::path::Path) {
        for dir in CACHE_DIRS {
            std::fs::create_dir_all(profile.join(dir)).unwrap();
            std::fs::write(profile.join(dir).join("data"), b"x").unwrap();
        }
        for name in MUST_SURVIVE {
            std::fs::create_dir_all(profile.join(name)).unwrap();
            std::fs::write(profile.join(name).join("data"), b"secret").unwrap();
        }
    }

    #[test]
    fn only_caches_are_reported_out_of_a_profile() {
        let dir = tempfile::tempdir().unwrap();
        let known = known_for(dir.path());
        let profile = chrome_profile(&known);
        lay_out_profile(&profile);

        let reported: Vec<String> = ChromiumCache::roots(&known)
            .into_iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(!reported.is_empty(), "the caches should be found");
        for name in MUST_SURVIVE {
            assert!(
                !reported.iter().any(|r| r == name),
                "{name} is not a cache and must never be reported: {reported:?}"
            );
        }
        for name in reported {
            assert!(
                CACHE_DIRS.contains(&name.as_str()),
                "{name} is outside the cache list"
            );
        }
    }

    #[test]
    fn a_profile_is_recognised_by_name_and_nothing_else_is() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in ["Default", "Profile 1", "Profile 27", "Guest Profile"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        // Chrome keeps plenty beside the profiles; none of it is one.
        for name in ["Crashpad", "ShaderCache", "Safe Browsing", "System Profile"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("Default.txt"), b"not a directory").unwrap();

        let found: Vec<String> = profiles(root)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            found,
            vec!["Default", "Guest Profile", "Profile 1", "Profile 27"]
        );
    }

    #[test]
    fn each_platform_looks_where_that_platform_keeps_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let known = known_for(dir.path());
        let chrome = &CHROMIUM[0];

        // Windows keeps both the caches and the profile data under one `User Data` directory.
        let windows = profile_roots(chrome, &known, true);
        assert_eq!(windows.len(), 1);
        assert!(
            windows[0].ends_with("Google\\Chrome\\User Data"),
            "{windows:?}"
        );
        assert!(windows[0].starts_with(known.local_app_data.as_ref().unwrap()));

        // macOS splits them: the HTTP cache under Caches, the rest under Application Support.
        let mac = profile_roots(chrome, &known, false);
        assert_eq!(mac.len(), 2);
        assert!(mac[0].starts_with(known.user_cache.as_ref().unwrap()));
        assert!(mac[1].starts_with(known.app_support.as_ref().unwrap()));
    }

    #[test]
    fn firefox_takes_the_cache_and_leaves_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        let known = known_for(dir.path());
        let profile = firefox_profile(&known);
        std::fs::create_dir_all(profile.join("cache2/entries")).unwrap();
        // The things a Firefox profile keeps that are not cache.
        for name in ["storage", "sessionstore-backups", "bookmarkbackups"] {
            std::fs::create_dir_all(profile.join(name)).unwrap();
        }

        let roots = FirefoxCache::roots(&known);
        assert_eq!(roots.len(), 1, "{roots:?}");
        assert!(roots[0].1.ends_with("cache2"), "{:?}", roots[0].1);
    }

    #[test]
    fn a_profile_without_a_cache_yields_nothing_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let known = known_for(dir.path());
        let profile = chrome_profile(&known);
        for name in MUST_SURVIVE {
            std::fs::create_dir_all(profile.join(name)).unwrap();
        }

        assert!(ChromiumCache::roots(&known).is_empty());
    }
}
