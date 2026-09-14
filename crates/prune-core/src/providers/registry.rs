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
    by_id: HashMap<&'static str, Arc<dyn CleanupProvider>>,
}

impl ProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn CleanupProvider>>) -> Self {
        let by_id = providers.iter().map(|p| (p.id(), Arc::clone(p))).collect();
        Self {
            ordered: providers,
            by_id,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_providers())
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
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
