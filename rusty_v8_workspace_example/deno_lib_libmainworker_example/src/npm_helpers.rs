// Helper functions copied from deno cli/rt/run.rs
use std::sync::Arc;

use url::Url;

// Create a default npmrc configuration
pub fn create_default_npmrc() -> Arc<deno_npm::npm_rc::ResolvedNpmRc> {
  Arc::new(deno_npm::npm_rc::ResolvedNpmRc {
    default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
      registry_url: Url::parse("https://registry.npmjs.org").unwrap(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  })
}
