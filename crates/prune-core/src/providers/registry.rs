use std::collections::HashMap;
use std::sync::Arc;

use super::{browser, developer, system, CleanupProvider};
use crate::models::ProviderInfo;
use crate::platform::KnownPaths;

/// The built-in provider set, in display order.
pub fn default_providers() -> Vec<Arc<dyn CleanupProvider>> {
    let mut v: Vec<Arc<dyn CleanupProvider>> = vec![
        Arc::new(system::USER_CACHE),
        Arc::new(system::USER_LOGS),
        Arc::new(system::TEMP_FILES),
        Arc::new(system::TRASH),
        Arc::new(system::OldInstallers),
        Arc::new(browser::ChromiumCache),
        Arc::new(browser::FirefoxCache),
    ];
    for p in developer::ALL {
        v.push(Arc::new(clone_simple(p)));
    }
    v.push(Arc::new(developer::ProjectArtifacts));
    v
}

// `SimpleProvider` holds only `'static` data and a fn pointer, so a field-wise copy is cheap.
fn clone_simple(p: &super::SimpleProvider) -> super::SimpleProvider {
    super::SimpleProvider {
        id: p.id,
        name: p.name,
        category: p.category,
        description: p.description,
        risk: p.risk,
        priority: p.priority,
        roots: p.roots,
        exclude_names: p.exclude_names,
        min_age_days: p.min_age_days,
        permanent_only: p.permanent_only,
    }
}

/// Lookup table over providers.
pub struct ProviderRegistry {
    ordered: Vec<Arc<dyn CleanupProvider>>,
    by_id: HashMap<String, Arc<dyn CleanupProvider>>,
}

impl ProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn CleanupProvider>>) -> Self {
        let by_id = providers
            .iter()
            .map(|p| (p.id().to_string(), Arc::clone(p)))
            .collect();
        Self {
            ordered: providers,
            by_id,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_providers())
    }

    /// The built-in providers plus the ones the user defined in `providers.json`.
    ///
    /// Custom providers are appended, so a built-in always keeps its id; `custom::load` refuses
    /// one that tries to take it.
    pub fn with_custom(custom: Vec<crate::providers::CustomProvider>) -> Self {
        let mut providers = default_providers();
        providers.extend(
            custom
                .into_iter()
                .map(|p| Arc::new(p) as Arc<dyn CleanupProvider>),
        );
        Self::new(providers)
    }

    /// Ids the user's file may not reuse.
    pub fn built_in_ids() -> Vec<String> {
        default_providers()
            .iter()
            .map(|p| p.id().to_string())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn CleanupProvider>> {
        self.by_id.get(id)
    }

    pub fn all(&self) -> &[Arc<dyn CleanupProvider>] {
        &self.ordered
    }

    pub fn infos(&self, known: &KnownPaths) -> Vec<ProviderInfo> {
        self.ordered
            .iter()
            .map(|p| ProviderInfo {
                id: p.id().to_string(),
                name: p.name().to_string(),
                category: p.category(),
                description: p.description().to_string(),
                default_risk: p.default_risk(),
                available: p.is_available(known),
                custom: p.is_custom(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_provider_joins_the_built_in_ones_and_outranks_the_generic_cache() {
        let loaded = crate::providers::custom::parse(
            r#"{"providers":[{"id":"mine","name":"Mine","category":"developer_files",
                "risk":"safe","paths":["~/mine"]}]}"#,
            &ProviderRegistry::built_in_ids()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        assert!(loaded.issues.is_empty(), "{:?}", loaded.issues);

        let registry = ProviderRegistry::with_custom(loaded.providers);
        let mine = registry
            .get("mine")
            .expect("the custom provider is registered");

        assert!(mine.is_custom());
        // Above `user_cache`, so a location someone named by hand is reported under that name
        // rather than as an anonymous cache folder.
        assert!(mine.priority() > registry.get("user_cache").unwrap().priority());
        // The built-in ones are all still there.
        assert!(registry.get("npm_cache").is_some());
    }

    #[test]
    fn built_in_ids_are_what_a_custom_file_may_not_reuse() {
        let ids = ProviderRegistry::built_in_ids();
        assert!(ids.iter().any(|id| id == "npm_cache"));
        assert_eq!(ids.len(), default_providers().len());
    }

    #[test]
    fn provider_ids_are_unique() {
        let providers = default_providers();
        let mut ids: Vec<&str> = providers.iter().map(|p| p.id()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate provider id");
    }
}
