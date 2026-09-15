//! Global caches written by package managers and build tools. Each is a single well-known
//! location, so they are all declarative [`SimpleProvider`]s.

use std::path::PathBuf;

use crate::models::{Category, RiskLevel};
use crate::platform::KnownPaths;
use crate::providers::{Root, SimpleProvider};

const DEV: Category = Category::DeveloperFiles;

fn local(known: &KnownPaths, rel: &str) -> Option<PathBuf> {
    known.local_app_data.as_ref().map(|l| l.join(rel))
}

fn cache(known: &KnownPaths, rel: &str) -> Option<PathBuf> {
    known.user_cache.as_ref().map(|c| c.join(rel))
}

fn whole(paths: impl IntoIterator<Item = Option<PathBuf>>) -> Vec<Root> {
    paths.into_iter().flatten().map(Root::whole).collect()
}

/// Expands one level: `~/.gem/ruby/*/cache` style layouts where the middle component is a
/// version number that cannot be known in advance.
fn glob_one_level(parent: PathBuf, leaf: &str) -> Vec<Root> {
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return Vec::new();
    };
    let mut roots: Vec<Root> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| p.join(leaf))
        .filter(|p| p.is_dir())
        .map(Root::whole)
        .collect();
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots
}

pub const NPM_CACHE: SimpleProvider = SimpleProvider::new(
    "npm_cache",
    "npm cache",
    DEV,
    "Downloaded package tarballs (~/.npm/_cacache). npm re-downloads what it needs.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "npm-cache")])
        } else {
            whole([Some(k.home.join(".npm/_cacache"))])
        }
    },
);

pub const PNPM_STORE: SimpleProvider = SimpleProvider::new(
    "pnpm_store",
    "pnpm store",
    DEV,
    "Content-addressable pnpm store plus its registry metadata and dlx cache. Existing \
     node_modules keep working; new installs re-fetch.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([
                local(k, "pnpm\\store"),
                Some(k.home.join(".pnpm-store")),
                local(k, "pnpm-cache"),
            ])
        } else {
            whole([
                Some(k.home.join("Library/pnpm/store")),
                Some(k.home.join(".local/share/pnpm/store")),
                Some(k.home.join(".pnpm-store")),
                // Registry metadata and `pnpm dlx` downloads; separate from the store and
                // often a few gigabytes on their own.
                cache(k, "pnpm"),
                Some(k.home.join(".cache/pnpm")),
            ])
        }
    },
);

pub const YARN_CACHE: SimpleProvider = SimpleProvider::new(
    "yarn_cache",
    "Yarn cache",
    DEV,
    "Yarn classic and Berry global caches.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([
                local(k, "Yarn\\Cache"),
                Some(k.home.join(".yarn/berry/cache")),
            ])
        } else {
            whole([cache(k, "Yarn"), Some(k.home.join(".yarn/berry/cache"))])
        }
    },
);

pub const BUN_CACHE: SimpleProvider = SimpleProvider::new(
    "bun_cache",
    "Bun cache",
    DEV,
    "Bun's global install cache.",
    RiskLevel::Safe,
    |k| whole([Some(k.home.join(".bun/install/cache"))]),
);

pub const PIP_CACHE: SimpleProvider = SimpleProvider::new(
    "pip_cache",
    "pip cache",
    DEV,
    "Downloaded wheels and HTTP cache for pip.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "pip\\cache")])
        } else {
            whole([cache(k, "pip"), Some(k.home.join(".cache/pip"))])
        }
    },
);

pub const UV_CACHE: SimpleProvider = SimpleProvider::new(
    "uv_cache",
    "uv cache",
    DEV,
    "Cache of the uv Python package manager.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "uv\\cache")])
        } else {
            whole([cache(k, "uv"), Some(k.home.join(".cache/uv"))])
        }
    },
);

pub const GRADLE_CACHE: SimpleProvider = SimpleProvider::new(
    "gradle_cache",
    "Gradle caches",
    DEV,
    "Dependency and build caches in ~/.gradle. Next build re-downloads dependencies.",
    RiskLevel::Low,
    |k| {
        vec![
            Root::labelled(k.home.join(".gradle/caches"), "Gradle caches"),
            Root::labelled(k.home.join(".gradle/daemon"), "Gradle daemon logs"),
            Root::labelled(
                k.home.join(".gradle/wrapper/dists"),
                "Gradle wrapper distributions",
            ),
        ]
    },
);

pub const MAVEN_REPO: SimpleProvider = SimpleProvider::new(
    "maven_repo",
    "Maven repository",
    DEV,
    "Local Maven repository (~/.m2/repository). Re-downloaded on the next build.",
    RiskLevel::Low,
    |k| whole([Some(k.home.join(".m2/repository"))]),
);

pub const CARGO_CACHE: SimpleProvider = SimpleProvider::new(
    "cargo_cache",
    "Cargo registry",
    DEV,
    "Downloaded crates and git checkouts in ~/.cargo. Cargo re-fetches on demand.",
    RiskLevel::Low,
    |k| {
        vec![
            Root::labelled(k.home.join(".cargo/registry/cache"), "Crate archives"),
            Root::labelled(
                k.home.join(".cargo/registry/src"),
                "Extracted crate sources",
            ),
            Root::labelled(
                k.home.join(".cargo/git/checkouts"),
                "Git dependency checkouts",
            ),
        ]
    },
);

pub const GO_CACHE: SimpleProvider = SimpleProvider::new(
    "go_cache",
    "Go caches",
    DEV,
    "Go build cache and module download cache.",
    RiskLevel::Low,
    |k| {
        let build = if cfg!(windows) {
            local(k, "go-build")
        } else {
            cache(k, "go-build")
        };
        let mut roots = Vec::new();
        if let Some(b) = build {
            roots.push(Root::labelled(b, "Go build cache"));
        }
        roots.push(Root::labelled(
            k.home.join("go/pkg/mod/cache/download"),
            "Go module downloads",
        ));
        roots
    },
);

pub const NUGET_CACHE: SimpleProvider = SimpleProvider::new(
    "nuget_cache",
    "NuGet packages",
    DEV,
    "Global NuGet package cache.",
    RiskLevel::Low,
    |k| whole([Some(k.home.join(".nuget/packages"))]),
);

pub const COCOAPODS_CACHE: SimpleProvider = SimpleProvider::new(
    "cocoapods_cache",
    "CocoaPods cache",
    DEV,
    "Downloaded pod specs and sources.",
    RiskLevel::Safe,
    |k| whole([cache(k, "CocoaPods")]),
);

pub const HOMEBREW_CACHE: SimpleProvider = SimpleProvider::new(
    "homebrew_cache",
    "Homebrew cache",
    DEV,
    "Downloaded bottles and casks. Equivalent to `brew cleanup`.",
    RiskLevel::Safe,
    |k| whole([cache(k, "Homebrew")]),
);

pub const XCODE_DERIVED_DATA: SimpleProvider = SimpleProvider::new(
    "xcode_derived_data",
    "Xcode DerivedData",
    DEV,
    "Intermediate build products and indexes, per project. Xcode rebuilds them.",
    RiskLevel::Safe,
    |k| {
        vec![Root::children(
            k.home.join("Library/Developer/Xcode/DerivedData"),
        )]
    },
);

pub const XCODE_ARCHIVES: SimpleProvider = SimpleProvider::new(
    "xcode_archives",
    "Xcode Archives",
    DEV,
    "App archives (.xcarchive). Needed to symbolicate crash logs of shipped builds.",
    RiskLevel::Medium,
    |k| {
        vec![Root::children(
            k.home.join("Library/Developer/Xcode/Archives"),
        )]
    },
);

pub const XCODE_DEVICE_SUPPORT: SimpleProvider = SimpleProvider::new(
    "xcode_device_support",
    "Xcode Device Support",
    DEV,
    "Debug symbols for iOS/watchOS/tvOS versions you connected. Re-created on next connect.",
    RiskLevel::Low,
    |k| {
        let x = k.home.join("Library/Developer/Xcode");
        vec![
            Root::children(x.join("iOS DeviceSupport")),
            Root::children(x.join("watchOS DeviceSupport")),
            Root::children(x.join("tvOS DeviceSupport")),
        ]
    },
);

pub const SIMULATOR_CACHES: SimpleProvider = SimpleProvider::new(
    "simulator_caches",
    "Simulator caches",
    DEV,
    "CoreSimulator caches (dyld shared caches etc.). Simulators and their data are untouched.",
    RiskLevel::Safe,
    |k| whole([Some(k.home.join("Library/Developer/CoreSimulator/Caches"))]),
);

pub const NODE_GYP_CACHE: SimpleProvider = SimpleProvider::new(
    "node_gyp_cache",
    "node-gyp headers",
    DEV,
    "Node headers and libraries downloaded to build native addons.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "node-gyp"), Some(k.home.join(".node-gyp"))])
        } else {
            whole([cache(k, "node-gyp"), Some(k.home.join(".node-gyp"))])
        }
    },
);

pub const DENO_CACHE: SimpleProvider = SimpleProvider::new(
    "deno_cache",
    "Deno cache",
    DEV,
    "Downloaded modules and compiled artifacts. Deno re-fetches what a program imports.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "deno")])
        } else {
            whole([cache(k, "deno"), Some(k.home.join(".cache/deno"))])
        }
    },
);

pub const PLAYWRIGHT_BROWSERS: SimpleProvider = SimpleProvider::new(
    "playwright_browsers",
    "Playwright browsers",
    DEV,
    "Chromium, Firefox and WebKit builds downloaded for tests. `playwright install` brings \
     them back, which takes a while.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([local(k, "ms-playwright")])
        } else {
            whole([
                cache(k, "ms-playwright"),
                Some(k.home.join(".cache/ms-playwright")),
            ])
        }
    },
);

pub const PUPPETEER_BROWSERS: SimpleProvider = SimpleProvider::new(
    "puppeteer_browsers",
    "Puppeteer browsers",
    DEV,
    "Chrome builds downloaded by Puppeteer. Re-downloaded on the next install.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([local(k, "puppeteer")])
        } else {
            whole([cache(k, "puppeteer"), Some(k.home.join(".cache/puppeteer"))])
        }
    },
);

pub const CYPRESS_BINARIES: SimpleProvider = SimpleProvider::new(
    "cypress_binaries",
    "Cypress binaries",
    DEV,
    "Downloaded Cypress test runner versions.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([local(k, "Cypress\\Cache")])
        } else {
            whole([cache(k, "Cypress"), Some(k.home.join(".cache/Cypress"))])
        }
    },
);

pub const ELECTRON_CACHE: SimpleProvider = SimpleProvider::new(
    "electron_cache",
    "Electron downloads",
    DEV,
    "Electron runtimes and electron-builder's download cache.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "electron"), local(k, "electron-builder")])
        } else {
            whole([cache(k, "electron"), cache(k, "electron-builder")])
        }
    },
);

pub const SWIFTPM_CACHE: SimpleProvider = SimpleProvider::new(
    "swiftpm_cache",
    "Swift Package Manager cache",
    DEV,
    "Cached package manifests and repositories. Re-fetched on the next resolve.",
    RiskLevel::Safe,
    |k| whole([cache(k, "org.swift.swiftpm")]),
);

pub const PUB_CACHE: SimpleProvider = SimpleProvider::new(
    "pub_cache",
    "Dart and Flutter packages",
    DEV,
    "Packages downloaded by pub. `flutter pub get` re-downloads them.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([local(k, "Pub\\Cache\\hosted"), local(k, "Pub\\Cache\\git")])
        } else {
            whole([
                Some(k.home.join(".pub-cache/hosted")),
                Some(k.home.join(".pub-cache/git")),
            ])
        }
    },
);

pub const RUBYGEMS_CACHE: SimpleProvider = SimpleProvider::new(
    "rubygems_cache",
    "RubyGems cache",
    DEV,
    "Downloaded .gem archives and Bundler's cache. Installed gems are left alone.",
    RiskLevel::Safe,
    |k| {
        // `~/.gem/ruby/<version>/cache`: only the downloaded archives, never the installed
        // gems next to them.
        let mut roots = glob_one_level(k.home.join(".gem/ruby"), "cache");
        roots.extend(whole([Some(k.home.join(".bundle/cache"))]));
        roots
    },
);

pub const COMPOSER_CACHE: SimpleProvider = SimpleProvider::new(
    "composer_cache",
    "Composer cache",
    DEV,
    "PHP packages downloaded by Composer.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "Composer")])
        } else {
            whole([
                Some(k.home.join(".composer/cache")),
                Some(k.home.join(".cache/composer")),
            ])
        }
    },
);

pub const JETBRAINS_CACHE: SimpleProvider = SimpleProvider::new(
    "jetbrains_cache",
    "JetBrains IDE caches",
    DEV,
    "Indexes and local history for IntelliJ, WebStorm, PyCharm and friends. Rebuilt when a \
     project is next opened.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            local(k, "JetBrains")
                .map(Root::children)
                .into_iter()
                .collect()
        } else {
            let mut roots: Vec<Root> = cache(k, "JetBrains")
                .map(Root::children)
                .into_iter()
                .collect();
            roots.extend(
                k.user_logs
                    .as_ref()
                    .map(|l| Root::children(l.join("JetBrains"))),
            );
            roots
        }
    },
);

pub const DOTSLASH_CACHE: SimpleProvider = SimpleProvider::new(
    "dotslash_cache",
    "DotSlash cache",
    DEV,
    "Executables DotSlash downloaded on demand. Re-fetched when next run.",
    RiskLevel::Safe,
    |k| {
        if cfg!(windows) {
            whole([local(k, "dotslash")])
        } else {
            whole([cache(k, "dotslash"), Some(k.home.join(".cache/dotslash"))])
        }
    },
);

/// All global tool caches, in display order.
pub const ALL: &[&SimpleProvider] = &[
    &NPM_CACHE,
    &PNPM_STORE,
    &YARN_CACHE,
    &BUN_CACHE,
    &NODE_GYP_CACHE,
    &DENO_CACHE,
    &PLAYWRIGHT_BROWSERS,
    &PUPPETEER_BROWSERS,
    &CYPRESS_BINARIES,
    &ELECTRON_CACHE,
    &PIP_CACHE,
    &UV_CACHE,
    &GRADLE_CACHE,
    &MAVEN_REPO,
    &CARGO_CACHE,
    &GO_CACHE,
    &NUGET_CACHE,
    &COCOAPODS_CACHE,
    &SWIFTPM_CACHE,
    &PUB_CACHE,
    &RUBYGEMS_CACHE,
    &COMPOSER_CACHE,
    &JETBRAINS_CACHE,
    &DOTSLASH_CACHE,
    &HOMEBREW_CACHE,
    &XCODE_DERIVED_DATA,
    &XCODE_ARCHIVES,
    &XCODE_DEVICE_SUPPORT,
    &SIMULATOR_CACHES,
];
