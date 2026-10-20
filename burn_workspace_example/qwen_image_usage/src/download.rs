use std::{
  error::Error,
  fs::{self, OpenOptions},
  io::{self, Write},
  path::{Path, PathBuf},
};

type DownloadResult<T> = Result<T, Box<dyn Error>>;

const DEFAULT_REVISION: &str = "d26bb61231c349cf6b7896fa83353113880e1ba3";

fn file_url(endpoint: &str, model: &str, name: &str) -> DownloadResult<url::Url> {
  let mut url = url::Url::parse(endpoint)?;
  {
    let mut path = url
      .path_segments_mut()
      .map_err(|_| "HF_ENDPOINT must be a base URL")?;
    path.pop_if_empty();
    let revision = if model == "Qwen/Qwen-Image-2.1" {
      DEFAULT_REVISION
    } else {
      "main"
    };
    for segment in model
      .split('/')
      .chain(["resolve", revision])
      .chain(name.split('/'))
    {
      if segment.is_empty() || segment == "." || segment == ".." {
        return Err("invalid model ID or filename".into());
      }
      path.push(segment);
    }
  }
  Ok(url)
}

fn expected_length(response: &ureq::http::Response<impl Sized>) -> DownloadResult<u64> {
  let header = response
    .headers()
    .get("content-length")
    .ok_or("missing Content-Length")?;
  Ok(header.to_str()?.parse()?)
}

fn content_range(
  response: &ureq::http::Response<impl Sized>,
) -> DownloadResult<Option<(u64, u64, u64)>> {
  let Some(value) = response.headers().get("content-range") else {
    return Ok(None);
  };
  let text = value.to_str()?;
  let Some((range, total)) = text
    .strip_prefix("bytes ")
    .and_then(|value| value.split_once('/'))
  else {
    return Err(format!("invalid Content-Range: {text}").into());
  };
  let Some((start, end)) = range.split_once('-') else {
    return Err(format!("invalid Content-Range: {text}").into());
  };
  let total = total
    .parse::<u64>()
    .map_err(|_| format!("invalid Content-Range total: {text}"))?;
  Ok(Some((
    start
      .parse::<u64>()
      .map_err(|_| format!("invalid Content-Range start: {text}"))?,
    end
      .parse::<u64>()
      .map_err(|_| format!("invalid Content-Range end: {text}"))?,
    total,
  )))
}

pub fn get_file(model: &str, name: &str, cache_dir: &Path) -> DownloadResult<PathBuf> {
  let endpoint = std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".into());
  let url = file_url(&endpoint, model, name)?;
  let model_path = model.replace('/', "--");
  let path = cache_dir.join(model_path).join(name);
  let length = expected_length(&ureq::head(url.as_str()).call()?)?;
  if path
    .metadata()
    .is_ok_and(|metadata| metadata.len() == length)
  {
    return Ok(path);
  }
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  let partial = path.with_extension(format!(
    "{}.part",
    path.extension().unwrap_or_default().to_string_lossy()
  ));
  let have = partial.metadata().map(|m| m.len()).unwrap_or(0);
  if have == length {
    fs::rename(&partial, &path)?;
    return Ok(path);
  }

  // HEAD is only a hint here. Some mirrors return a stale or truncated
  // Content-Length for very large files; the GET response's Content-Range
  // total is authoritative when resuming.
  let mut request = ureq::get(url.as_str());
  if have > 0 {
    request = request.header("Range", format!("bytes={have}-"));
  }
  let mut response = request
    .config()
    .http_status_as_error(false)
    .build()
    .call()?;
  if response.status().as_u16() == 416 {
    let total = response
      .headers()
      .get("content-range")
      .and_then(|value| value.to_str().ok())
      .and_then(|value| value.strip_prefix("bytes */"))
      .and_then(|value| value.parse::<u64>().ok());
    if total == Some(have) {
      fs::rename(&partial, &path)?;
      return Ok(path);
    }
    eprintln!(
      "Restarting {name}: local partial size {have} does not match remote size {}",
      total.map_or_else(|| "unknown".into(), |value| value.to_string())
    );
    response = ureq::get(url.as_str()).call()?;
  }
  let range = content_range(&response)?;
  let resumable = if response.status().as_u16() == 206 {
    let Some((start, _, _)) = range else {
      return Err("server returned 206 without Content-Range".into());
    };
    if start != have {
      return Err(format!("invalid resume offset: requested {have}, received {start}").into());
    }
    true
  } else {
    false
  };
  let target_length = range
    .and_then(|(_, _, total)| (total > 0).then_some(total))
    .or_else(|| {
      response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok()?.parse().ok())
    })
    .unwrap_or(length);
  let mut file = OpenOptions::new()
    .create(true)
    .write(true)
    .append(resumable)
    .truncate(!resumable)
    .open(&partial)?;
  let copied = io::copy(&mut response.body_mut().as_reader(), &mut file)?;
  file.flush()?;
  let actual = partial.metadata()?.len();
  if actual != target_length {
    return Err(
      format!("incomplete download of {name}: {actual}/{target_length} bytes (received {copied})")
        .into(),
    );
  }
  fs::rename(&partial, &path)?;
  Ok(path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builds_encoded_model_url() {
    let url = file_url(
      "https://hf-mirror.com/",
      "Qwen/Qwen-Image-2.1",
      "processor/tokenizer.json",
    )
    .unwrap();
    assert!(url.as_str().contains("Qwen/Qwen-Image-2.1/resolve/"));
    assert!(url.as_str().ends_with("processor/tokenizer.json"));
    assert!(file_url("https://hf-mirror.com", "Qwen/../X", "x").is_err());
  }

  #[test]
  fn parses_content_range_total() {
    let response = ureq::http::Response::builder()
      .status(206)
      .header("Content-Range", "bytes 9968332504-11714643987/11714643988")
      .body(())
      .unwrap();
    assert_eq!(
      content_range(&response).unwrap(),
      Some((9968332504, 11714643987, 11714643988))
    );
  }

  #[test]
  #[ignore = "downloads a small model manifest from HF_ENDPOINT"]
  fn mirror_downloads_model_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let file = get_file("Qwen/Qwen-Image-2.1", "model_index.json", temp.path()).unwrap();
    assert!(file.metadata().unwrap().len() > 0);
    assert_eq!(
      file,
      get_file("Qwen/Qwen-Image-2.1", "model_index.json", temp.path()).unwrap()
    );
  }

  #[test]
  #[ignore = "downloads the Qwen-Image-2.1 tokenizer from HF_ENDPOINT"]
  fn mirror_downloads_tokenizer() {
    let path = get_file(
      "Qwen/Qwen-Image-2.1",
      "processor/tokenizer.json",
      Path::new(".model-cache/qwen-image-2.1"),
    )
    .unwrap();
    let tokenizer = tokenizers::Tokenizer::from_file(&path).unwrap();
    assert!(tokenizer.get_vocab_size(false) > 100_000);
  }
}
