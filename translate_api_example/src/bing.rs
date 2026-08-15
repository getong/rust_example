//! Free Bing translator, no API key required.
//!
//! Same flow as gt.el's gt-engine-bing.el: fetch https://www.bing.com/translator
//! once to scrape the session credentials (IG, key, token, regional host), then
//! POST the text to /ttranslatev3.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub struct BingTranslator {
  client: Client,
  host: String,
  ig: String,
  key: String,
  token: String,
}

impl BingTranslator {
  pub fn new() -> Result<Self> {
    let client = Client::builder()
      .user_agent(USER_AGENT)
      .timeout(Duration::from_secs(20))
      .build()
      .context("failed to build http client")?;

    let page = client
      .get("https://www.bing.com/translator")
      .send()
      .context("failed to fetch bing translator page")?
      .error_for_status()?
      .text()?;

    let ig = extract(&page, "IG:\"", "\"").context("IG not found in bing page")?;

    // curUrl="https://www.bing.com/..." — keep only scheme + host, the
    // redirected regional host (e.g. cn.bing.com) must be reused for the
    // translate request or the token is rejected.
    let cur_url = extract(&page, "curUrl=\"", "\"").context("curUrl not found in bing page")?;
    let host = cur_url
      .find("bing.com")
      .map(|end| format!("{}bing.com", &cur_url[.. end]))
      .context("no bing.com host in curUrl")?;

    // var params_AbusePreventionHelper = [123456, "token", ...]
    let helper = extract(&page, "params_AbusePreventionHelper = [", "]")
      .context("abuse prevention params not found in bing page")?;
    let mut parts = helper.split(',');
    let key = parts
      .next()
      .map(str::trim)
      .filter(|key| !key.is_empty())
      .context("bing key missing")?
      .to_string();
    let token = parts
      .next()
      .map(|part| part.trim().trim_matches('"'))
      .filter(|token| !token.is_empty())
      .context("bing token missing")?
      .to_string();

    Ok(Self {
      client,
      host,
      ig,
      key,
      token,
    })
  }

  pub fn translate(&self, text: &str, source: &str, target: &str) -> Result<String> {
    let response = self
      .client
      .post(format!(
        "{}/ttranslatev3?isVertical=1&IG={}&IID=translator.5022.1",
        self.host, self.ig
      ))
      .form(&[
        ("key", self.key.as_str()),
        ("token", self.token.as_str()),
        ("text", text),
        ("fromLang", &bing_lang(source)),
        ("to", &bing_lang(target)),
      ])
      .send()
      .context("bing translate request failed")?;

    if response.status().as_u16() == 429 {
      bail!("bing: too many requests, please try later");
    }

    let body: serde_json::Value = response.error_for_status()?.json()?;
    body[0]["translations"][0]["text"]
      .as_str()
      .map(str::to_string)
      .ok_or_else(|| anyhow!("unexpected bing response: {body}"))
  }
}

fn bing_lang(lang: &str) -> String {
  match lang {
    "auto" => "auto-detect".into(),
    "zh" | "zh-CN" => "zh-Hans".into(),
    "zh-TW" => "zh-Hant".into(),
    other => other.into(),
  }
}

fn extract(haystack: &str, start: &str, end: &str) -> Option<String> {
  let from = haystack.find(start)? + start.len();
  let len = haystack[from ..].find(end)?;
  Some(haystack[from .. from + len].to_string())
}
