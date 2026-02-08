// Copyright 2018-2026 the Deno authors. MIT license.

use std::{
  borrow::Cow,
  collections::HashSet,
  env,
  net::SocketAddr,
  num::{NonZeroU8, NonZeroU32, NonZeroUsize},
  path::{Path, PathBuf},
  str::FromStr,
  sync::Arc,
};

use deno_bundle_runtime::{BundleFormat, BundlePlatform, PackageHandling, SourceMapType};
use deno_config::{
  deno_json::{NewestDependencyDate, NodeModulesDirMode},
  glob::{FilePatterns, PathOrPatternSet},
};
use deno_core::{anyhow::Context, error::AnyError, url::Url};
use deno_graph::GraphKind;
use deno_lib::args::{CaData, UnstableConfig};
use deno_npm::NpmSystemInfo;
use deno_npm_installer::PackagesAllowedScripts;
use deno_path_util::{normalize_path, resolve_url_or_path, url_to_file_path};
pub use deno_runtime::deno_inspector_server::InspectPublishUid;
use deno_telemetry::{OtelConfig, OtelConsoleConfig, OtelPropagators};
use log::Level;
use serde::{Deserialize, Serialize};

use crate::util::fs::canonicalize_path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ConfigFlag {
  #[default]
  Discover,
  Path(String),
  Disabled,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileFlags {
  pub ignore: Vec<String>,
  pub include: Vec<String>,
}

impl FileFlags {
  pub fn as_file_patterns(&self, base: &Path) -> Result<FilePatterns, AnyError> {
    Ok(FilePatterns {
      include: if self.include.is_empty() {
        None
      } else {
        Some(PathOrPatternSet::from_include_relative_path_or_patterns(
          base,
          &self.include,
        )?)
      },
      exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(base, &self.ignore)?,
      base: base.to_path_buf(),
    })
  }
}

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum DefaultRegistry {
  Npm,
  Jsr,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddFlags {
  pub packages: Vec<String>,
  pub dev: bool,
  pub default_registry: Option<DefaultRegistry>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuditFlags {
  pub severity: String,
  pub ignore_registry_errors: bool,
  pub ignore_unfixable: bool,
  pub dev: bool,
  pub prod: bool,
  pub optional: bool,
  pub ignore: Vec<String>,
  pub socket: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RemoveFlags {
  pub packages: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchFlags {
  pub files: FileFlags,
  pub filter: Option<String>,
  pub json: bool,
  pub no_run: bool,
  pub permit_no_files: bool,
  pub watch: Option<WatchFlags>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheFlags {
  pub files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckFlags {
  pub files: Vec<String>,
  pub doc: bool,
  pub doc_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileFlags {
  pub source_file: String,
  pub output: Option<String>,
  pub args: Vec<String>,
  pub target: Option<String>,
  pub no_terminal: bool,
  pub icon: Option<String>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub eszip: bool,
}

impl CompileFlags {
  pub fn resolve_target(&self) -> String {
    self
      .target
      .clone()
      .unwrap_or_else(|| env!("TARGET").to_string())
  }
}

#[derive(Clone)]
pub enum CompletionsFlags {
  Static(Box<[u8]>),
  Dynamic(Arc<dyn Fn() -> Result<(), AnyError> + Send + Sync + 'static>),
}

impl PartialEq for CompletionsFlags {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Static(l0), Self::Static(r0)) => l0 == r0,
      (Self::Dynamic(l0), Self::Dynamic(r0)) => Arc::ptr_eq(l0, r0),
      _ => false,
    }
  }
}

impl Eq for CompletionsFlags {}

impl std::fmt::Debug for CompletionsFlags {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Static(arg0) => f.debug_tuple("Static").field(arg0).finish(),
      Self::Dynamic(_) => f.debug_tuple("Dynamic").finish(),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum CoverageType {
  #[default]
  Summary,
  Detailed,
  Lcov,
  Html,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CoverageFlags {
  pub files: FileFlags,
  pub output: Option<String>,
  pub include: Vec<String>,
  pub exclude: Vec<String>,
  pub r#type: CoverageType,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct DeployFlags {
  pub sandbox: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum DocSourceFileFlag {
  #[default]
  Builtin,
  Paths(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocHtmlFlag {
  pub name: Option<String>,
  pub category_docs_path: Option<String>,
  pub symbol_redirect_map_path: Option<String>,
  pub default_symbol_map_path: Option<String>,
  pub strip_trailing_html: bool,
  pub output: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocFlags {
  pub private: bool,
  pub json: bool,
  pub lint: bool,
  pub html: Option<DocHtmlFlag>,
  pub source_files: DocSourceFileFlag,
  pub filter: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalFlags {
  pub print: bool,
  pub code: String,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct FmtFlags {
  pub check: bool,
  pub files: FileFlags,
  pub permit_no_files: bool,
  pub use_tabs: Option<bool>,
  pub line_width: Option<NonZeroU32>,
  pub indent_width: Option<NonZeroU8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<String>,
  pub no_semicolons: Option<bool>,
  pub watch: Option<WatchFlags>,
  pub unstable_component: bool,
  pub unstable_sql: bool,
}

impl FmtFlags {
  pub fn is_stdin(&self) -> bool {
    let args = &self.files.include;
    args.len() == 1 && args[0] == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitFlags {
  pub package: Option<String>,
  pub package_args: Vec<String>,
  pub dir: Option<String>,
  pub lib: bool,
  pub serve: bool,
  pub empty: bool,
  pub yes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfoFlags {
  pub json: bool,
  pub file: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFlagsGlobal {
  pub module_urls: Vec<String>,
  pub args: Vec<String>,
  pub name: Option<String>,
  pub root: Option<String>,
  pub force: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlags {
  Local(InstallFlagsLocal),
  Global(InstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallFlagsLocal {
  Add(AddFlags),
  TopLevel(InstallTopLevelFlags),
  Entrypoints(InstallEntrypointsFlags),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallTopLevelFlags {
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallEntrypointsFlags {
  pub entrypoints: Vec<String>,
  pub lockfile_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JSONReferenceFlags {
  pub json: deno_core::serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JupyterFlags {
  pub install: bool,
  pub name: Option<String>,
  pub display: Option<String>,
  pub kernel: bool,
  pub conn_file: Option<String>,
  pub force: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFlagsGlobal {
  pub name: String,
  pub root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UninstallKind {
  Local(RemoveFlags),
  Global(UninstallFlagsGlobal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallFlags {
  pub kind: UninstallKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintFlags {
  pub files: FileFlags,
  pub rules: bool,
  pub fix: bool,
  pub maybe_rules_tags: Option<Vec<String>>,
  pub maybe_rules_include: Option<Vec<String>>,
  pub maybe_rules_exclude: Option<Vec<String>>,
  pub permit_no_files: bool,
  pub json: bool,
  pub compact: bool,
  pub watch: Option<WatchFlags>,
}

impl LintFlags {
  pub fn is_stdin(&self) -> bool {
    let args = &self.files.include;
    args.len() == 1 && args[0] == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ReplFlags {
  pub eval_files: Option<Vec<String>>,
  pub eval: Option<String>,
  pub is_default_command: bool,
  pub json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct RunFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub bare: bool,
  pub coverage_dir: Option<String>,
  pub print_task_list: bool,
}

impl RunFlags {
  #[cfg(test)]
  pub fn new_default(script: String) -> Self {
    Self {
      script,
      watch: None,
      bare: false,
      coverage_dir: None,
      print_task_list: false,
    }
  }

  pub fn is_stdin(&self) -> bool {
    self.script == "-"
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum DenoXShimName {
  #[default]
  Dx,
  Denox,
  Dnx,
  Other(String),
}

impl DenoXShimName {
  pub fn name(&self) -> &str {
    match self {
      Self::Dx => "dx",
      Self::Denox => "denox",
      Self::Dnx => "dnx",
      Self::Other(name) => name,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XFlagsKind {
  InstallAlias(DenoXShimName),
  Command(XCommandFlags),
  Print,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XCommandFlags {
  pub yes: bool,
  pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XFlags {
  pub kind: XFlagsKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeFlags {
  pub script: String,
  pub watch: Option<WatchFlagsWithPaths>,
  pub port: u16,
  pub host: String,
  pub parallel: bool,
  pub open_site: bool,
}

impl ServeFlags {
  #[cfg(test)]
  pub fn new_default(script: String, port: u16, host: &str) -> Self {
    Self {
      script,
      watch: None,
      port,
      host: host.to_owned(),
      parallel: false,
      open_site: false,
    }
  }
}

pub enum WatchFlagsRef<'a> {
  Watch(&'a WatchFlags),
  WithPaths(&'a WatchFlagsWithPaths),
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct WatchFlags {
  pub hmr: bool,
  pub no_clear_screen: bool,
  pub exclude: Vec<String>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct WatchFlagsWithPaths {
  pub hmr: bool,
  pub paths: Vec<String>,
  pub no_clear_screen: bool,
  pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskFlags {
  pub cwd: Option<String>,
  pub task: Option<String>,
  pub is_run: bool,
  pub recursive: bool,
  pub filter: Option<String>,
  pub eval: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TestReporterConfig {
  #[default]
  Pretty,
  Dot,
  Junit,
  Tap,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestFlags {
  pub doc: bool,
  pub no_run: bool,
  pub coverage_dir: Option<String>,
  pub coverage_raw_data_only: bool,
  pub clean: bool,
  pub fail_fast: Option<NonZeroUsize>,
  pub files: FileFlags,
  pub parallel: bool,
  pub permit_no_files: bool,
  pub filter: Option<String>,
  pub shuffle: Option<u64>,
  pub trace_leaks: bool,
  pub watch: Option<WatchFlagsWithPaths>,
  pub reporter: TestReporterConfig,
  pub junit_path: Option<String>,
  pub hide_stacktraces: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeFlags {
  pub dry_run: bool,
  pub force: bool,
  pub release_candidate: bool,
  pub canary: bool,
  pub version: Option<String>,
  pub output: Option<String>,
  pub version_or_hash_or_channel: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishFlags {
  pub token: Option<String>,
  pub dry_run: bool,
  pub allow_slow_types: bool,
  pub allow_dirty: bool,
  pub no_provenance: bool,
  pub set_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpFlags {
  pub help: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanFlags {
  pub except_paths: Vec<String>,
  pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleFlags {
  pub entrypoints: Vec<String>,
  pub output_path: Option<String>,
  pub output_dir: Option<String>,
  pub external: Vec<String>,
  pub format: BundleFormat,
  pub minify: bool,
  pub code_splitting: bool,
  pub inline_imports: bool,
  pub packages: PackageHandling,
  pub sourcemap: Option<SourceMapType>,
  pub platform: BundlePlatform,
  pub watch: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DenoSubcommand {
  Add(AddFlags),
  Audit(AuditFlags),
  ApproveScripts(ApproveScriptsFlags),
  Remove(RemoveFlags),
  Bench(BenchFlags),
  Bundle(BundleFlags),
  Cache(CacheFlags),
  Check(CheckFlags),
  Clean(CleanFlags),
  Compile(CompileFlags),
  Completions(CompletionsFlags),
  Coverage(CoverageFlags),
  Deploy(DeployFlags),
  Doc(DocFlags),
  Eval(EvalFlags),
  Fmt(FmtFlags),
  Init(InitFlags),
  Info(InfoFlags),
  Install(InstallFlags),
  JSONReference(JSONReferenceFlags),
  Jupyter(JupyterFlags),
  Uninstall(UninstallFlags),
  Lsp,
  Lint(LintFlags),
  Repl(ReplFlags),
  Run(RunFlags),
  Serve(ServeFlags),
  Task(TaskFlags),
  Test(TestFlags),
  Outdated(OutdatedFlags),
  Types,
  Upgrade(UpgradeFlags),
  Vendor,
  Publish(PublishFlags),
  Help(HelpFlags),
  X(XFlags),
}

impl DenoSubcommand {
  pub fn watch_flags(&self) -> Option<WatchFlagsRef<'_>> {
    match self {
      Self::Run(RunFlags {
        watch: Some(flags), ..
      })
      | Self::Test(TestFlags {
        watch: Some(flags), ..
      }) => Some(WatchFlagsRef::WithPaths(flags)),
      Self::Bench(BenchFlags {
        watch: Some(flags), ..
      })
      | Self::Lint(LintFlags {
        watch: Some(flags), ..
      })
      | Self::Fmt(FmtFlags {
        watch: Some(flags), ..
      }) => Some(WatchFlagsRef::Watch(flags)),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutdatedKind {
  Update {
    latest: bool,
    interactive: bool,
    lockfile_only: bool,
  },
  PrintOutdated {
    compatible: bool,
  },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutdatedFlags {
  pub filters: Vec<String>,
  pub recursive: bool,
  pub kind: OutdatedKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApproveScriptsFlags {
  pub lockfile_only: bool,
  pub packages: Vec<String>,
}

impl DenoSubcommand {
  pub fn is_run(&self) -> bool {
    matches!(self, Self::Run(_))
  }

  // Returns `true` if the subcommand depends on testing infrastructure.
  pub fn needs_test(&self) -> bool {
    matches!(
      self,
      Self::Test(_) | Self::Jupyter(_) | Self::Repl(_) | Self::Bench(_) | Self::Lint(_) | Self::Lsp
    )
  }

  pub fn npm_system_info(&self) -> NpmSystemInfo {
    match self {
      DenoSubcommand::Compile(CompileFlags {
        target: Some(target),
        ..
      }) => {
        // the values of NpmSystemInfo align with the possible values for the
        // `arch` and `platform` fields of Node.js' `process` global:
        // https://nodejs.org/api/process.html
        match target.as_str() {
          "aarch64-apple-darwin" => NpmSystemInfo {
            os: "darwin".into(),
            cpu: "arm64".into(),
          },
          "aarch64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".into(),
            cpu: "arm64".into(),
          },
          "x86_64-apple-darwin" => NpmSystemInfo {
            os: "darwin".into(),
            cpu: "x64".into(),
          },
          "x86_64-unknown-linux-gnu" => NpmSystemInfo {
            os: "linux".into(),
            cpu: "x64".into(),
          },
          "x86_64-pc-windows-msvc" => NpmSystemInfo {
            os: "win32".into(),
            cpu: "x64".into(),
          },
          value => {
            log::warn!(
              concat!(
                "Not implemented npm system info for target '{}'. Using current ",
                "system default. This may impact architecture specific dependencies."
              ),
              value,
            );
            NpmSystemInfo::default()
          }
        }
      }
      _ => {
        let arch = std::env::var_os("DENO_INSTALL_ARCH");
        if let Some(var) = arch.as_ref().and_then(|s| s.to_str()) {
          NpmSystemInfo::from_rust(std::env::consts::OS, var)
        } else {
          NpmSystemInfo::default()
        }
      }
    }
  }
}

impl Default for DenoSubcommand {
  fn default() -> DenoSubcommand {
    DenoSubcommand::Repl(ReplFlags {
      eval_files: None,
      eval: None,
      is_default_command: true,
      json: false,
    })
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum TypeCheckMode {
  /// Type-check all modules.
  All,
  /// Skip type-checking of all modules. The default value for "deno run" and
  /// several other subcommands.
  #[default]
  None,
  /// Only type-check local modules. The default value for "deno test" and
  /// several other subcommands.
  Local,
}

impl TypeCheckMode {
  /// Gets if type checking will occur under this mode.
  pub fn is_true(&self) -> bool {
    match self {
      Self::None => false,
      Self::Local | Self::All => true,
    }
  }

  /// Gets the corresponding module `GraphKind` that should be created
  /// for the current `TypeCheckMode`.
  pub fn as_graph_kind(&self) -> GraphKind {
    match self.is_true() {
      true => GraphKind::All,
      false => GraphKind::CodeOnly,
    }
  }
}

/// Parse --inspect-publish-uid from a comma-separated string like "stderr,http".
pub fn parse_inspect_publish_uid(s: &str) -> Result<InspectPublishUid, String> {
  let mut result = InspectPublishUid {
    console: false,
    http: false,
  };
  for part in s.split(',') {
    let part = part.trim();
    match part {
      "stderr" => result.console = true,
      "http" => result.http = true,
      "" => {}
      _ => {
        return Err(format!(
          "--inspect-publish-uid destination can be stderr or http, got '{}'",
          part
        ));
      }
    }
  }
  Ok(result)
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct InternalFlags {
  /// Used when the language server is configured with an
  /// explicit cache option.
  pub cache_path: Option<PathBuf>,
  /// Override the path to use for the node_modules directory.
  pub root_node_modules_dir_override: Option<PathBuf>,
  /// Only reads to the lockfile instead of writing to it.
  pub lockfile_skip_write: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Flags {
  pub initial_cwd: Option<PathBuf>,
  /// Vector of CLI arguments - these are user script arguments, all Deno
  /// specific flags are removed.
  pub argv: Vec<String>,
  pub subcommand: DenoSubcommand,

  pub frozen_lockfile: Option<bool>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<CaData>,
  pub cache_blocklist: Vec<String>,
  pub cached_only: bool,
  pub type_check_mode: TypeCheckMode,
  pub config_flag: ConfigFlag,
  pub node_modules_dir: Option<NodeModulesDirMode>,
  pub vendor: Option<bool>,
  pub enable_op_summary_metrics: bool,
  pub enable_testing_features: bool,
  pub ext: Option<String>,
  /// Flags that aren't exposed in the CLI, but are used internally.
  pub internal: InternalFlags,
  pub ignore: Vec<String>,
  pub import_map_path: Option<String>,
  pub env_file: Option<Vec<String>>,
  pub inspect_brk: Option<SocketAddr>,
  pub inspect_wait: Option<SocketAddr>,
  pub inspect: Option<SocketAddr>,
  pub inspect_publish_uid: Option<InspectPublishUid>,
  pub location: Option<Url>,
  pub lock: Option<String>,
  pub log_level: Option<Level>,
  pub minimum_dependency_age: Option<NewestDependencyDate>,
  pub no_remote: bool,
  pub no_lock: bool,
  pub no_npm: bool,
  pub reload: bool,
  pub seed: Option<u64>,
  pub trace_ops: Option<Vec<String>>,
  pub unstable_config: UnstableConfig,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub v8_flags: Vec<String>,
  pub code_cache_enabled: bool,
  pub permissions: PermissionFlags,
  pub allow_scripts: PackagesAllowedScripts,
  pub permission_set: Option<String>,
  pub eszip: bool,
  pub node_conditions: Vec<String>,
  pub preload: Vec<String>,
  pub require: Vec<String>,
  pub tunnel: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct PermissionFlags {
  pub allow_all: bool,
  pub allow_env: Option<Vec<String>>,
  pub deny_env: Option<Vec<String>>,
  pub ignore_env: Option<Vec<String>>,
  pub allow_ffi: Option<Vec<String>>,
  pub deny_ffi: Option<Vec<String>>,
  pub allow_net: Option<Vec<String>>,
  pub deny_net: Option<Vec<String>>,
  pub allow_read: Option<Vec<String>>,
  pub deny_read: Option<Vec<String>>,
  pub ignore_read: Option<Vec<String>>,
  pub allow_run: Option<Vec<String>>,
  pub deny_run: Option<Vec<String>>,
  pub allow_sys: Option<Vec<String>>,
  pub deny_sys: Option<Vec<String>>,
  pub allow_write: Option<Vec<String>>,
  pub deny_write: Option<Vec<String>>,
  pub no_prompt: bool,
  pub allow_import: Option<Vec<String>>,
  pub deny_import: Option<Vec<String>>,
}

impl PermissionFlags {
  pub fn has_permission(&self) -> bool {
    self.allow_all
      || self.allow_env.is_some()
      || self.deny_env.is_some()
      || self.ignore_env.is_some()
      || self.allow_ffi.is_some()
      || self.deny_ffi.is_some()
      || self.allow_net.is_some()
      || self.deny_net.is_some()
      || self.allow_read.is_some()
      || self.deny_read.is_some()
      || self.ignore_read.is_some()
      || self.allow_run.is_some()
      || self.deny_run.is_some()
      || self.allow_sys.is_some()
      || self.deny_sys.is_some()
      || self.allow_write.is_some()
      || self.deny_write.is_some()
      || self.allow_import.is_some()
      || self.deny_import.is_some()
  }
}

fn join_paths(allowlist: &[String], d: &str) -> String {
  allowlist
    .iter()
    .map(|path| path.to_string())
    .collect::<Vec<String>>()
    .join(d)
}

impl Flags {
  /// Return list of permission arguments that are equivalent
  /// to the ones used to create `self`.
  pub fn to_permission_args(&self) -> Vec<String> {
    let mut args = vec![];

    if self.permissions.allow_all {
      args.push("--allow-all".to_string());
      return args;
    }

    match &self.permissions.allow_read {
      Some(read_allowlist) if read_allowlist.is_empty() => {
        args.push("--allow-read".to_string());
      }
      Some(read_allowlist) => {
        let s = format!("--allow-read={}", join_paths(read_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_read {
      Some(read_denylist) if read_denylist.is_empty() => {
        args.push("--deny-read".to_string());
      }
      Some(read_denylist) => {
        let s = format!("--deny-read={}", join_paths(read_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_write {
      Some(write_allowlist) if write_allowlist.is_empty() => {
        args.push("--allow-write".to_string());
      }
      Some(write_allowlist) => {
        let s = format!("--allow-write={}", join_paths(write_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_write {
      Some(write_denylist) if write_denylist.is_empty() => {
        args.push("--deny-write".to_string());
      }
      Some(write_denylist) => {
        let s = format!("--deny-write={}", join_paths(write_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_net {
      Some(net_allowlist) if net_allowlist.is_empty() => {
        args.push("--allow-net".to_string());
      }
      Some(net_allowlist) => {
        let s = format!("--allow-net={}", net_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_net {
      Some(net_denylist) if net_denylist.is_empty() => {
        args.push("--deny-net".to_string());
      }
      Some(net_denylist) => {
        let s = format!("--deny-net={}", net_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.unsafely_ignore_certificate_errors {
      Some(ic_allowlist) if ic_allowlist.is_empty() => {
        args.push("--unsafely-ignore-certificate-errors".to_string());
      }
      Some(ic_allowlist) => {
        let s = format!(
          "--unsafely-ignore-certificate-errors={}",
          ic_allowlist.join(",")
        );
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_env {
      Some(env_allowlist) if env_allowlist.is_empty() => {
        args.push("--allow-env".to_string());
      }
      Some(env_allowlist) => {
        let s = format!("--allow-env={}", env_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_env {
      Some(env_denylist) if env_denylist.is_empty() => {
        args.push("--deny-env".to_string());
      }
      Some(env_denylist) => {
        let s = format!("--deny-env={}", env_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.ignore_env {
      Some(ignorelist) if ignorelist.is_empty() => {
        args.push("--ignore-env".to_string());
      }
      Some(ignorelist) => {
        let s = format!("--ignore-env={}", ignorelist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.ignore_read {
      Some(ignorelist) if ignorelist.is_empty() => {
        args.push("--ignore-read".to_string());
      }
      Some(ignorelist) => {
        let s = format!("--ignore-read={}", ignorelist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_run {
      Some(run_allowlist) if run_allowlist.is_empty() => {
        args.push("--allow-run".to_string());
      }
      Some(run_allowlist) => {
        let s = format!("--allow-run={}", run_allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_run {
      Some(run_denylist) if run_denylist.is_empty() => {
        args.push("--deny-run".to_string());
      }
      Some(run_denylist) => {
        let s = format!("--deny-run={}", run_denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_sys {
      Some(sys_allowlist) if sys_allowlist.is_empty() => {
        args.push("--allow-sys".to_string());
      }
      Some(sys_allowlist) => {
        let s = format!("--allow-sys={}", sys_allowlist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.deny_sys {
      Some(sys_denylist) if sys_denylist.is_empty() => {
        args.push("--deny-sys".to_string());
      }
      Some(sys_denylist) => {
        let s = format!("--deny-sys={}", sys_denylist.join(","));
        args.push(s)
      }
      _ => {}
    }

    match &self.permissions.allow_ffi {
      Some(ffi_allowlist) if ffi_allowlist.is_empty() => {
        args.push("--allow-ffi".to_string());
      }
      Some(ffi_allowlist) => {
        let s = format!("--allow-ffi={}", join_paths(ffi_allowlist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_ffi {
      Some(ffi_denylist) if ffi_denylist.is_empty() => {
        args.push("--deny-ffi".to_string());
      }
      Some(ffi_denylist) => {
        let s = format!("--deny-ffi={}", join_paths(ffi_denylist, ","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.allow_import {
      Some(allowlist) if allowlist.is_empty() => {
        args.push("--allow-import".to_string());
      }
      Some(allowlist) => {
        let s = format!("--allow-import={}", allowlist.join(","));
        args.push(s);
      }
      _ => {}
    }

    match &self.permissions.deny_import {
      Some(denylist) if denylist.is_empty() => {
        args.push("--deny-import".to_string());
      }
      Some(denylist) => {
        let s = format!("--deny-import={}", denylist.join(","));
        args.push(s);
      }
      _ => {}
    }

    args
  }

  pub fn no_legacy_abort(&self) -> bool {
    self
      .unstable_config
      .features
      .contains(&String::from("no-legacy-abort"))
  }

  pub fn otel_config(&self) -> OtelConfig {
    let otel_var = |name| match std::env::var(name) {
      Ok(s) if s.eq_ignore_ascii_case("true") => Some(true),
      Ok(s) if s.eq_ignore_ascii_case("false") => Some(false),
      Ok(_) => {
        log::warn!("'{name}' env var value not recognized, only 'true' and 'false' are accepted");
        None
      }
      Err(_) => None,
    };

    let disabled = otel_var("OTEL_SDK_DISABLED").unwrap_or(false);
    let default = !disabled && otel_var("OTEL_DENO").unwrap_or(false);

    let propagators = if default {
      if let Ok(propagators) = std::env::var("OTEL_PROPAGATORS") {
        propagators
          .split(',')
          .filter_map(|p| match p.trim() {
            "tracecontext" => Some(OtelPropagators::TraceContext),
            "baggage" => Some(OtelPropagators::Baggage),
            _ => None,
          })
          .collect()
      } else {
        HashSet::from([OtelPropagators::TraceContext, OtelPropagators::Baggage])
      }
    } else {
      HashSet::default()
    };

    OtelConfig {
      tracing_enabled: !disabled && otel_var("OTEL_DENO_TRACING").unwrap_or(default),
      metrics_enabled: !disabled && otel_var("OTEL_DENO_METRICS").unwrap_or(default),
      propagators,
      console: match std::env::var("OTEL_DENO_CONSOLE").as_deref() {
        Ok(_) if disabled => OtelConsoleConfig::Ignore,
        Ok("ignore") => OtelConsoleConfig::Ignore,
        Ok("capture") => OtelConsoleConfig::Capture,
        Ok("replace") => OtelConsoleConfig::Replace,
        res => {
          if res.is_ok() {
            log::warn!(
              "'OTEL_DENO_CONSOLE' env var value not recognized, only 'ignore', 'capture', or \
               'replace' are accepted"
            );
          }
          if default {
            OtelConsoleConfig::Capture
          } else {
            OtelConsoleConfig::Ignore
          }
        }
      },
      deterministic_prefix: std::env::var("DENO_UNSTABLE_OTEL_DETERMINISTIC")
        .as_deref()
        .map(u8::from_str)
        .map(|x| match x {
          Ok(x) => Some(x),
          Err(_) => {
            log::warn!(
              "'DENO_UNSTABLE_OTEL_DETERMINISTIC' env var value not recognized, only integers are \
               accepted"
            );
            None
          }
        })
        .ok()
        .flatten(),
    }
  }

  /// Extract the paths the config file should be discovered from.
  ///
  /// Returns `None` if the config file should not be auto-discovered.
  pub fn config_path_args(&self, current_dir: &Path) -> Option<Vec<PathBuf>> {
    fn resolve_multiple_files(files_or_dirs: &[String], current_dir: &Path) -> Vec<PathBuf> {
      let mut seen = HashSet::with_capacity(files_or_dirs.len());
      let result = files_or_dirs
        .iter()
        .filter_map(|p| {
          let path = normalize_path(Cow::Owned(current_dir.join(p)));
          if seen.insert(path.clone()) {
            Some(path.into_owned())
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      if result.is_empty() {
        vec![current_dir.to_path_buf()]
      } else {
        result
      }
    }

    fn resolve_single_folder_path(
      arg: &str,
      current_dir: &Path,
      maybe_resolve_directory: impl FnOnce(PathBuf) -> Option<PathBuf>,
    ) -> Option<PathBuf> {
      if let Ok(module_specifier) = resolve_url_or_path(arg, current_dir) {
        if module_specifier.scheme() == "file" || module_specifier.scheme() == "npm" {
          if let Ok(p) = url_to_file_path(&module_specifier) {
            maybe_resolve_directory(p)
          } else {
            Some(current_dir.to_path_buf())
          }
        } else {
          // When the entrypoint doesn't have file: scheme (it's the remote
          // script), then we don't auto discover the config file.
          None
        }
      } else {
        Some(current_dir.to_path_buf())
      }
    }

    use DenoSubcommand::*;
    match &self.subcommand {
      Fmt(FmtFlags { files, .. }) => Some(resolve_multiple_files(&files.include, current_dir)),
      Lint(LintFlags { files, .. }) => Some(resolve_multiple_files(&files.include, current_dir)),
      Run(RunFlags { script, .. })
      | Compile(CompileFlags {
        source_file: script,
        ..
      }) => resolve_single_folder_path(
        script,
        current_dir,
        |mut p| {
          if p.pop() { Some(p) } else { None }
        },
      )
      .map(|p| vec![p]),
      Task(TaskFlags {
        cwd: Some(path), ..
      }) => {
        // todo(dsherret): Why is this canonicalized? Document why.
        // attempt to resolve the config file from the task subcommand's
        // `--cwd` when specified
        match canonicalize_path(Path::new(path)) {
          Ok(path) => Some(vec![path]),
          Err(_) => Some(vec![current_dir.to_path_buf()]),
        }
      }
      Cache(CacheFlags { files, .. })
      | Install(InstallFlags::Local(InstallFlagsLocal::Entrypoints(InstallEntrypointsFlags {
        entrypoints: files,
        ..
      }))) => Some(vec![
        files
          .iter()
          .filter_map(|file| {
            resolve_single_folder_path(file, current_dir, |mut p| {
              if p.is_dir() {
                return Some(p);
              }
              if p.pop() { Some(p) } else { None }
            })
          })
          .next()
          .unwrap_or_else(|| current_dir.to_path_buf()),
      ]),
      _ => Some(vec![current_dir.to_path_buf()]),
    }
  }

  pub fn has_permission(&self) -> bool {
    self.permissions.has_permission()
  }

  pub fn has_permission_in_argv(&self) -> bool {
    self.argv.iter().any(|arg| {
      arg == "--allow-all"
        || arg.starts_with("--allow-env")
        || arg.starts_with("--deny-env")
        || arg.starts_with("--allow-ffi")
        || arg.starts_with("--deny-ffi")
        || arg.starts_with("--allow-net")
        || arg.starts_with("--deny-net")
        || arg.starts_with("--allow-read")
        || arg.starts_with("--deny-read")
        || arg.starts_with("--allow-run")
        || arg.starts_with("--deny-run")
        || arg.starts_with("--allow-sys")
        || arg.starts_with("--deny-sys")
        || arg.starts_with("--allow-write")
        || arg.starts_with("--deny-write")
    })
  }

  pub fn resolve_watch_exclude_set(&self) -> Result<PathOrPatternSet, AnyError> {
    match self.subcommand.watch_flags() {
      Some(WatchFlagsRef::WithPaths(WatchFlagsWithPaths {
        exclude: excluded_paths,
        ..
      }))
      | Some(WatchFlagsRef::Watch(WatchFlags {
        exclude: excluded_paths,
        ..
      })) => {
        let cwd = std::env::current_dir()?;
        PathOrPatternSet::from_exclude_relative_path_or_patterns(&cwd, excluded_paths)
          .context("Failed resolving watch exclude patterns.")
      }
      _ => Ok(PathOrPatternSet::default()),
    }
  }
}

/// Usage text used by `tools::upgrade` help output.
pub static UPGRADE_USAGE: &str = r#"<g>Latest</>
  <bold>deno upgrade</>

<g>Specific version</>
  <bold>deno upgrade</> <p(245)>1.45.0</>
  <bold>deno upgrade</> <p(245)>1.46.0-rc.1</>
  <bold>deno upgrade</> <p(245)>9bc2dd29ad6ba334fd57a20114e367d3c04763d4</>

<g>Channel</>
  <bold>deno upgrade</> <p(245)>stable</>
  <bold>deno upgrade</> <p(245)>rc</>
  <bold>deno upgrade</> <p(245)>canary</>"#;
