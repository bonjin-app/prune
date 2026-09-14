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
    "Content-addressable pnpm store. Existing node_modules keep working; new installs re-fetch.",
    RiskLevel::Low,
    |k| {
        if cfg!(windows) {
            whole([local(k, "pnpm\\store"), Some(k.home.join(".pnpm-store"))])
        } else {
            whole([
                Some(k.home.join("Library/pnpm/store")),
                Some(k.home.join(".local/share/pnpm/store")),
                Some(k.home.join(".pnpm-store")),
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

/// All global tool caches, in display order.
pub const ALL: &[&SimpleProvider] = &[
    &NPM_CACHE,
    &PNPM_STORE,
    &YARN_CACHE,
    &BUN_CACHE,
    &PIP_CACHE,
    &UV_CACHE,
    &GRADLE_CACHE,
    &MAVEN_REPO,
    &CARGO_CACHE,
    &GO_CACHE,
    &NUGET_CACHE,
    &COCOAPODS_CACHE,
    &HOMEBREW_CACHE,
    &XCODE_DERIVED_DATA,
    &XCODE_ARCHIVES,
    &XCODE_DEVICE_SUPPORT,
    &SIMULATOR_CACHES,
];
