mod bing;

use std::env;

use anyhow::{Context, Result, ensure};
use translators::{GoogleTranslator, Translator};

const USAGE: &str =
  "usage: translate_api_example [--engine google|bing] <target-language> <text>";

#[derive(Clone, Copy, PartialEq)]
enum Engine {
  Google,
  Bing,
}

fn main() -> Result<()> {
  let mut args = env::args().skip(1).peekable();

  let mut engine = Engine::Google;
  if args.peek().map(String::as_str) == Some("--engine") {
    args.next();
    engine = match args.next().as_deref() {
      Some("google") => Engine::Google,
      Some("bing") => Engine::Bing,
      _ => anyhow::bail!(USAGE),
    };
  }

  let target_language = args.next().context(USAGE)?;
  let text = args.collect::<Vec<_>>().join(" ");
  ensure!(!text.is_empty(), USAGE);

  let translated = match translate(engine, &text, &target_language) {
    Ok(translated) => translated,
    Err(err) => {
      let fallback = if engine == Engine::Google {
        Engine::Bing
      } else {
        Engine::Google
      };
      eprintln!(
        "warning: {err:#}, falling back to {}",
        engine_name(fallback)
      );
      translate(fallback, &text, &target_language)
        .with_context(|| format!("{} fallback also failed", engine_name(fallback)))?
    }
  };

  println!("{translated}");
  Ok(())
}

fn translate(engine: Engine, text: &str, target_language: &str) -> Result<String> {
  match engine {
    Engine::Google => google_translator()
      .translate_sync(text, "auto", google_lang(target_language))
      .map_err(|err| anyhow::anyhow!("google: {err}")),
    Engine::Bing => bing::BingTranslator::new()?.translate(text, "auto", target_language),
  }
}

// Google's free endpoint silently returns the text untranslated for the bare
// "zh" code; it only accepts region-qualified Chinese codes.
fn google_lang(lang: &str) -> &str {
  match lang {
    "zh" | "zh-Hans" => "zh-CN",
    "zh-Hant" => "zh-TW",
    other => other,
  }
}

fn engine_name(engine: Engine) -> &'static str {
  match engine {
    Engine::Google => "google",
    Engine::Bing => "bing",
  }
}

fn google_translator() -> GoogleTranslator {
  let builder = GoogleTranslator::builder()
    .timeout(20_usize)
    .text_limit(5000_usize);

  if let Some(proxy) = proxy_address() {
    return builder.proxy_address(proxy).build();
  }

  builder.build()
}

fn proxy_address() -> Option<String> {
  ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
    .into_iter()
    .find_map(|name| env::var(name).ok().filter(|value| !value.is_empty()))
}
