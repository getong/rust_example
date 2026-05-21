// Helper functions for npm resolution
use std::sync::Arc;

use deno_core::url::Url;

/// Create a default npmrc configuration pointing to the public npm registry.
pub(crate) fn create_default_npmrc() -> Arc<deno_npmrc::ResolvedNpmRc> {
  Arc::new(deno_npmrc::ResolvedNpmRc {
    default_config: deno_npmrc::RegistryConfigWithUrl {
      registry_url: Url::parse("https://registry.npmjs.org").unwrap(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  })
}
