use deno_core::{error::AnyError, FsModuleLoader, JsRuntime, RuntimeOptions};
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

const BOOTSTRAP_JS: &str = r#"
import { core } from "ext:core/mod.js";
import { Console } from "ext:deno_web/01_console.js";
import { fetch } from "ext:deno_fetch/26_fetch.js";
import { FormData } from "ext:deno_fetch/21_formdata.js";
import { Headers } from "ext:deno_fetch/20_headers.js";
import { Request } from "ext:deno_fetch/23_request.js";
import { Response } from "ext:deno_fetch/23_response.js";

globalThis.console = new Console(core.print);
globalThis.fetch = fetch;
globalThis.FormData = FormData;
globalThis.Headers = Headers;
globalThis.Request = Request;
globalThis.Response = Response;
"#;

const DENO_WEB_INIT_SPECIFIER: &str = "ext:deno_web/__init.js";
const DENO_WEB_INIT_JS: &str = r#"
import "ext:deno_web/00_infra.js";
import "ext:deno_web/01_dom_exception.js";
import "ext:deno_web/01_mimesniff.js";
import "ext:deno_web/02_event.js";
import "ext:deno_web/02_structured_clone.js";
import "ext:deno_web/02_timers.js";
import "ext:deno_web/03_abort_signal.js";
import "ext:deno_web/04_global_interfaces.js";
import "ext:deno_web/05_base64.js";
import "ext:deno_web/06_streams.js";
import "ext:deno_web/08_text_encoding.js";
import "ext:deno_web/09_file.js";
import "ext:deno_web/10_filereader.js";
import "ext:deno_web/12_location.js";
import "ext:deno_web/13_message_port.js";
import "ext:deno_web/14_compression.js";
import "ext:deno_web/15_performance.js";
import "ext:deno_web/16_image_data.js";
import "ext:deno_web/00_url.js";
import "ext:deno_web/01_urlpattern.js";
import "ext:deno_web/01_console.js";
import "ext:deno_web/01_broadcast_channel.js";
"#;

// The upstream `deno_net` ESM module `02_tls.js` expects an op
// (`op_tls_peer_certificate`) that is not available in this minimal embedder.
// `deno_fetch` only needs `loadTlsKeyPair()` for `Deno.createHttpClient()`, so
// we provide a small shim at the same `ext:` specifier.
const DENO_NET_TLS_SHIM_JS: &str = r#"
import { op_tls_key_null, op_tls_key_static } from "ext:core/ops";

export function loadTlsKeyPair(api, { keyFormat, cert, key }) {
  if (keyFormat !== undefined && keyFormat !== "pem") {
    throw new TypeError(
      `If \"keyFormat\" is specified, it must be \"pem\": received \"${keyFormat}\"`,
    );
  }

  if (cert !== undefined && key === undefined) {
    throw new TypeError(
      `If \`cert\` is specified, \`key\` must be specified as well for \`${api}\``,
    );
  }
  if (cert === undefined && key !== undefined) {
    throw new TypeError(
      `If \`key\` is specified, \`cert\` must be specified as well for \`${api}\``,
    );
  }

  if (cert !== undefined) {
    return op_tls_key_static(cert, key);
  }

  return op_tls_key_null();
}
"#;

// `deno_fetch` imports telemetry helpers from `ext:deno_telemetry/*`.
// In the upstream runtime these modules are TypeScript, but this example uses
// the plain `FsModuleLoader` (no TS transpilation). Provide tiny JS shims that
// keep tracing disabled.
const DENO_TELEMETRY_SHIM_JS: &str = r#"
export const TRACING_ENABLED = false;
export const PROPAGATORS = [];

export function builtinTracer() {
  return {
    startSpan() {
      return { end() {} };
    },
  };
}

export const ContextManager = {
  active() {
    return {};
  },
};

export function enterSpan(_span) {
  return null;
}

export function restoreSnapshot(_snapshot) {}
"#;

const DENO_TELEMETRY_UTIL_SHIM_JS: &str = r#"
export function updateSpanFromError(_span, _error) {}
export function updateSpanFromRequest(_span, _request) {}
export function updateSpanFromResponse(_span, _response) {}
"#;

const DENO_TELEMETRY_INIT_SPECIFIER: &str = "ext:deno_telemetry/__init.js";
const DENO_TELEMETRY_INIT_JS: &str = r#"
import "ext:deno_telemetry/telemetry.ts";
import "ext:deno_telemetry/util.ts";
"#;

const DENO_FETCH_INIT_SPECIFIER: &str = "ext:deno_fetch/__init.js";
const DENO_FETCH_INIT_JS: &str = r#"
import "ext:deno_fetch/20_headers.js";
import "ext:deno_fetch/21_formdata.js";
import "ext:deno_fetch/22_body.js";
import "ext:deno_fetch/22_http_client.js";
import "ext:deno_fetch/23_request.js";
import "ext:deno_fetch/23_response.js";
import "ext:deno_fetch/26_fetch.js";
import "ext:deno_fetch/27_eventsource.js";
"#;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
  // rustls 0.23 requires installing a process-level crypto provider.
  // Deno uses aws-lc by default, but multiple providers can be enabled via the
  // dependency graph, so we set it explicitly.
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),
    extensions: vec![
      {
        let mut ext = deno_webidl::deno_webidl::init();
        ext.esm_entry_point = Some("ext:deno_webidl/00_webidl.js");
        ext
      },
      {
        let mut ext = deno_web::deno_web::init(
          Default::default(), // BlobStore
          None,               // Base URL
          Default::default(), // InMemoryBroadcastChannel
        );
        ext.esm_files.to_mut().push(deno_core::ExtensionFileSource::new_computed(
          DENO_WEB_INIT_SPECIFIER,
          Arc::<str>::from(DENO_WEB_INIT_JS),
        ));
        ext.esm_entry_point = Some(DENO_WEB_INIT_SPECIFIER);
        ext
      },
      {
        let mut ext = deno_net::deno_net::init(None, None);
        ext.esm_files = Cow::Owned(vec![deno_core::ExtensionFileSource::new_computed(
          "ext:deno_net/02_tls.js",
          Arc::<str>::from(DENO_NET_TLS_SHIM_JS),
        )]);
        ext.lazy_loaded_esm_files = Cow::Owned(vec![]);
        ext.esm_entry_point = Some("ext:deno_net/02_tls.js");
        ext
      },
      {
        let mut ext = deno_core::Extension::default();
        ext.name = "telemetry_shim";
        ext.esm_files = Cow::Owned(vec![
          deno_core::ExtensionFileSource::new_computed(
            "ext:deno_telemetry/telemetry.ts",
            Arc::<str>::from(DENO_TELEMETRY_SHIM_JS),
          ),
          deno_core::ExtensionFileSource::new_computed(
            "ext:deno_telemetry/util.ts",
            Arc::<str>::from(DENO_TELEMETRY_UTIL_SHIM_JS),
          ),
          deno_core::ExtensionFileSource::new_computed(
            DENO_TELEMETRY_INIT_SPECIFIER,
            Arc::<str>::from(DENO_TELEMETRY_INIT_JS),
          ),
        ]);
        ext.esm_entry_point = Some(DENO_TELEMETRY_INIT_SPECIFIER);
        ext
      },
      {
        let mut ext = deno_fetch::deno_fetch::init(deno_fetch::Options {
          user_agent: "MyRuntime/1.0".to_string(),
          ..Default::default()
        });
        ext.esm_files.to_mut().push(deno_core::ExtensionFileSource::new_computed(
          DENO_FETCH_INIT_SPECIFIER,
          Arc::<str>::from(DENO_FETCH_INIT_JS),
        ));
        ext.esm_entry_point = Some(DENO_FETCH_INIT_SPECIFIER);
        ext
      },
    ],
    ..Default::default()
  });

  {
    let descriptor_parser = Arc::new(
      deno_permissions::RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys::default(),
      ),
    );
    let permissions =
      deno_permissions::PermissionsContainer::allow_all(descriptor_parser);

    let op_state = runtime.op_state();
    let mut state = op_state.borrow_mut();
    state.put(permissions);
    state.put::<Arc<deno_features::FeatureChecker>>(Arc::default());
  }

  let bootstrap_id = runtime
    .load_side_es_module_from_code(
      &deno_core::ModuleSpecifier::parse(
        "ext:deno_fetch_example/bootstrap.js",
      )?,
      BOOTSTRAP_JS,
    )
    .await?;
  let bootstrap_eval = runtime.mod_evaluate(bootstrap_id);

  let filepath = std::env::current_dir()?.join("script.ts");
  let main_module = deno_core::resolve_path(&filepath.to_string_lossy(), &std::env::current_dir()?)?;

  let mod_id = runtime.load_main_es_module(&main_module).await?;
  let mod_eval = runtime.mod_evaluate(mod_id);

  runtime.run_event_loop(Default::default()).await?;

  bootstrap_eval.await?;
  mod_eval.await?;

  Ok(())
}
