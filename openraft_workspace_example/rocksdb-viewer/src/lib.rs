use std::{io::Read, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::{Url, blocking::Client};

// Shared wire schema only: no server runtime or RocksDB dependency in the UI.
#[path = "../../openraft_libp2p_cluster/src/browser_protocol.rs"]
mod protocol;
pub use protocol::{Page, Row};
pub mod records;

const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Query {
  pub url: String,
  pub group_id: String,
  pub column_family: Option<String>,
  pub prefix: String,
  pub after: Option<String>,
}

fn endpoint(query: &Query) -> Result<Url> {
  let mut url =
    Url::parse(query.url.trim()).context("请输入完整的 http:// 或 https:// 节点 URL")?;
  if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
    bail!("节点 URL 必须使用 HTTP 或 HTTPS");
  }
  if url.query().is_some() || url.fragment().is_some() {
    bail!("请输入节点基础 URL，不要附加查询参数或片段");
  }
  url.set_path(&format!(
    "{}/rocksdb/entries",
    url.path().trim_end_matches('/')
  ));
  Ok(url)
}

/// Blocking HTTP request: invoke on GPUI's background executor, never in render.
/// No local database/filesystem access is performed by this client.
pub fn read_page(query: &Query) -> Result<Page> {
  let url = endpoint(query)?;
  let client = Client::builder()
    .connect_timeout(Duration::from_secs(5))
    .timeout(Duration::from_secs(15))
    .redirect(reqwest::redirect::Policy::none())
    .build()
    .context("创建 HTTP 客户端")?;
  let mut params = vec![("prefix", query.prefix.as_str())];
  if !query.group_id.trim().is_empty() {
    params.push(("group_id", query.group_id.trim()));
  }
  if let Some(cf) = &query.column_family {
    params.push(("cf", cf));
  }
  if let Some(after) = &query.after {
    params.push(("cursor", after));
  }
  let response = client
    .get(url)
    .query(&params)
    .send()
    .context("连接节点失败（请检查 URL、端口和节点是否启动）")?;
  let status = response.status();
  let mut body = Vec::new();
  response
    .take(MAX_RESPONSE_BYTES + 1)
    .read_to_end(&mut body)
    .context("读取节点响应")?;
  if body.len() as u64 > MAX_RESPONSE_BYTES {
    bail!("节点响应超过 4 MiB 限制");
  }
  if !status.is_success() {
    let message = String::from_utf8_lossy(&body[.. body.len().min(4096)]);
    bail!("节点返回 HTTP {status}：{message}");
  }
  serde_json::from_slice(&body).context("节点响应不是有效的 RocksDB 浏览数据，请确认服务端已更新")
}

#[cfg(test)]
mod tests {
  use std::{io::Write, net::TcpListener, thread};

  use super::*;

  fn query(url: String) -> Query {
    Query {
      url,
      group_id: "用户 & team".into(),
      column_family: Some("sm_data".into()),
      prefix: "a+b/&".into(),
      after: Some("AP8".into()),
    }
  }

  fn server(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_owned();
    let body = body.to_owned();
    let task = thread::spawn(move || {
      let (mut socket, _) = listener.accept().unwrap();
      socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
      let mut request = Vec::new();
      let mut byte = [0];
      while !request.ends_with(b"\r\n\r\n") {
        socket.read_exact(&mut byte).unwrap();
        request.push(byte[0]);
      }
      write!(
        socket,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n{body}",
        body.len()
      )
      .unwrap();
      String::from_utf8(request).unwrap()
    });
    (url, task)
  }

  #[test]
  fn fetches_json_and_encodes_filter_and_cursor() {
    let page = Page {
      group_id: "用户 & team".into(),
      column_families: vec!["sm_data".into()],
      column_family: "sm_data".into(),
      rows: vec![Row {
        key: "你好".into(),
        value: "hex: 00ff".into(),
        key_bytes: 6,
        value_bytes: 2,
      }],
      next: Some("next-cursor".into()),
    };
    let (url, server) = server("200 OK", &serde_json::to_string(&page).unwrap());
    let actual = read_page(&query(url)).unwrap();
    assert_eq!(actual.rows[0].key, "你好");
    assert_eq!(actual.next.as_deref(), Some("next-cursor"));
    let request = server.join().unwrap();
    let target = request.split_whitespace().nth(1).unwrap();
    let url = Url::parse(&format!("http://localhost{target}")).unwrap();
    assert_eq!(url.path(), "/rocksdb/entries");
    let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(params["prefix"], "a+b/&");
    assert_eq!(params["group_id"], "用户 & team");
    assert_eq!(params["cursor"], "AP8");
  }

  #[test]
  fn reports_http_and_invalid_payload_errors() {
    for (status, body, expected) in [
      ("404 Not Found", "group not hosted", "404"),
      ("200 OK", "not json", "不是有效"),
    ] {
      let (url, server) = server(status, body);
      assert!(
        read_page(&query(url))
          .unwrap_err()
          .to_string()
          .contains(expected)
      );
      server.join().unwrap();
    }
  }

  #[test]
  fn rejects_non_http_urls_and_preserves_proxy_prefix() {
    assert!(endpoint(&query("file:///tmp/db".into())).is_err());
    assert!(endpoint(&query("http://localhost/?cf=logs".into())).is_err());
    assert_eq!(
      endpoint(&query("https://example.com/node/".into()))
        .unwrap()
        .as_str(),
      "https://example.com/node/rocksdb/entries"
    );
  }
}
