use data_url::DataUrl;
use deno_core::{
  ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
  RequestedModuleType, ResolutionKind, anyhow::bail, futures::FutureExt, resolve_import,
};
use deno_error::JsErrorBox;

pub struct SimpleModuleLoader;

impl ModuleLoader for SimpleModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    resolve_import(specifier, referrer).map_err(|e| JsErrorBox::from_err(e))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    requested_module_type: RequestedModuleType,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();

    let fut = async move {
      let mut redirect_module_url = None;
      let bytes = match module_specifier.scheme() {
        "http" | "https" => {
          let res = reqwest::get(module_specifier.clone()).await?;
          // TODO: The HTML spec says to fail if the status is not
          // 200-299, but `error_for_status()` fails if the status is
          // 400-599. Redirect status codes are handled by reqwest,
          // but there are still status codes that are not handled.
          let res = res.error_for_status()?;
          // res.url() is the post-redirect URL.
          if res.url() != &module_specifier {
            redirect_module_url = Some(res.url().clone());
          }
          res.bytes().await?.to_vec()
        }
        "file" => {
          let path = match module_specifier.to_file_path() {
            Ok(path) => path,
            Err(_) => bail!("Invalid file URL."),
          };
          tokio::fs::read(path).await?
        }
        "data" => {
          let url = match DataUrl::process(module_specifier.as_str()) {
            Ok(url) => url,
            Err(_) => bail!("Not a valid data URL."),
          };
          match url.decode_to_vec() {
            Ok((bytes, _)) => bytes,
            Err(_) => bail!("Not a valid data URL."),
          }
        }
        schema => bail!("Invalid schema {}", schema),
      };

      // TODO: The MIME types should probably be checked.
      let module_type = match requested_module_type {
        RequestedModuleType::None => ModuleType::JavaScript,
        RequestedModuleType::Json => ModuleType::Json,
        RequestedModuleType::Other(_) => {
          unreachable!("Import types other than JSON are not supported")
        }
        _ => {
          unreachable!("Import types other than JSON are not supported")
        }
      };

      if let Some(redirect_module_url) = redirect_module_url {
        Ok(ModuleSource::new_with_redirect(
          module_type,
          ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
          &module_specifier,
          &redirect_module_url,
          None,
        ))
      } else {
        Ok(ModuleSource::new(
          module_type,
          ModuleSourceCode::Bytes(bytes.into_boxed_slice().into()),
          &module_specifier,
          None,
        ))
      }
    }
    .map(|r| r.map_err(|e| JsErrorBox::generic(e.to_string())))
    .boxed_local();

    ModuleLoadResponse::Async(fut)
  }
}
