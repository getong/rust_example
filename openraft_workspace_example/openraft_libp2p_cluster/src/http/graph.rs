use std::{collections::BTreeMap, sync::Arc};

use axum::{
  extract::{Query, State},
  http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderValue},
  },
  response::{IntoResponse, Response},
};
use openraft::async_runtime::WatchReceiver;

use super::{AppState, ClusterQuery, openraft_group_ids, remote_server_state};
use crate::graphviz::{
  ClusterGraphGroup, ClusterGraphNode, ClusterGraphSnapshot, cluster_graph_dot, cluster_graph_svg,
};

pub(super) async fn cluster_graph_page(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  let body = render_cluster_graph_page(&snapshot);
  (
    StatusCode::OK,
    [(
      CONTENT_TYPE,
      HeaderValue::from_static("text/html; charset=utf-8"),
    )],
    body,
  )
    .into_response()
}

pub(super) async fn cluster_graph_dot_response(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  (
    StatusCode::OK,
    [(
      CONTENT_TYPE,
      HeaderValue::from_static("text/vnd.graphviz; charset=utf-8"),
    )],
    cluster_graph_dot(&snapshot),
  )
    .into_response()
}

pub(super) async fn cluster_graph_svg_response(
  State(state): State<Arc<AppState>>,
  Query(query): Query<ClusterQuery>,
) -> Response {
  let snapshot = cluster_graph_snapshot(state.as_ref(), query).await;
  match tokio::task::spawn_blocking(move || cluster_graph_svg(&snapshot)).await {
    Ok(Ok(svg)) => (
      StatusCode::OK,
      [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
      svg,
    )
      .into_response(),
    Ok(Err(err)) => (
      StatusCode::INTERNAL_SERVER_ERROR,
      [(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
      )],
      format!("render graphviz svg: {err}"),
    )
      .into_response(),
    Err(err) => (
      StatusCode::INTERNAL_SERVER_ERROR,
      [(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
      )],
      format!("join graphviz render task: {err}"),
    )
      .into_response(),
  }
}

async fn cluster_graph_snapshot(state: &AppState, query: ClusterQuery) -> ClusterGraphSnapshot {
  // No (or empty) filter → render every raft group; each group contributes
  // its own leader and replication edges to the same physical topology.
  let selected = query.group_filter();
  let all_group_ids = openraft_group_ids(&state.registry);
  let render_ids = match selected.clone() {
    Some(group_id) => vec![group_id],
    None => all_group_ids.clone(),
  };

  let error = render_ids
    .is_empty()
    .then(|| "openraft groups are not initialized".to_string());

  let groups: Vec<ClusterGraphGroup> = render_ids
    .into_iter()
    .map(|group_id| match state.registry.get(&group_id) {
      Some(group) => ClusterGraphGroup {
        group_id,
        metrics: Some(group.raft.metrics().borrow_watched().clone()),
        error: None,
      },
      None => ClusterGraphGroup {
        error: Some(format!("unknown group_id={group_id}")),
        group_id,
        metrics: None,
      },
    })
    .collect();

  let known_nodes = state.network.known_nodes().await;
  let mut nodes = Vec::with_capacity(known_nodes.len());
  for (node_id, peer_id, addr) in known_nodes {
    let connected = state.network.is_peer_connected(&peer_id).await;
    let mut server_states = BTreeMap::new();
    if node_id == state.node_id {
      for group in &groups {
        if let Some(metrics) = group.metrics.as_ref() {
          server_states.insert(group.group_id.clone(), metrics.state);
        }
      }
    }
    nodes.push(ClusterGraphNode {
      node_id,
      peer_id: peer_id.to_string(),
      addr: addr.to_string(),
      connected,
      server_states,
    });
  }

  // Live server states of remote nodes, fetched per (group, node) pair in
  // parallel so the page pays one RPC round-trip, not groups x nodes.
  let mut probes = Vec::new();
  for group in &groups {
    if group.metrics.is_none() {
      continue;
    }
    for (index, node) in nodes.iter().enumerate() {
      if node.node_id == state.node_id || !node.connected {
        continue;
      }
      let group_id = group.group_id.clone();
      let node_id = node.node_id.clone();
      let network = state.network.clone();
      probes.push(async move {
        let server_state = remote_server_state(&group_id, &node_id, &network).await;
        (index, group_id, server_state)
      });
    }
  }
  for (index, group_id, server_state) in futures::future::join_all(probes).await {
    if let Some(server_state) = server_state {
      nodes[index].server_states.insert(group_id, server_state);
    }
  }

  ClusterGraphSnapshot {
    self_node_id: state.node_id.clone(),
    self_peer_id: state.peer_id.clone(),
    self_listen: state.listen.clone(),
    selected,
    all_group_ids,
    groups,
    nodes,
    error,
  }
}

fn render_cluster_graph_page(snapshot: &ClusterGraphSnapshot) -> String {
  let all_selected = if snapshot.selected.is_none() {
    " selected"
  } else {
    ""
  };
  let group_options = std::iter::once(format!(
    "<option value=\"\"{all_selected}>all groups</option>"
  ))
  .chain(snapshot.all_group_ids.iter().map(|group| {
    let selected = if snapshot.selected.as_deref() == Some(group.as_str()) {
      " selected"
    } else {
      ""
    };
    format!(
      "<option value=\"{}\"{}>{}</option>",
      html_escape(group),
      selected,
      html_escape(group)
    )
  }))
  .collect::<String>();
  let status = snapshot
    .error
    .iter()
    .chain(
      snapshot
        .groups
        .iter()
        .filter_map(|group| group.error.as_ref()),
    )
    .map(|err| format!("<p class=\"error\">{}</p>", html_escape(err)))
    .collect::<String>();
  // Empty group_id in links/SVG means "all groups" (the snapshot treats an
  // empty filter as no filter).
  let selected_param = snapshot.selected.as_deref().unwrap_or("");
  format!(
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="refresh" content="5">
  <title>libp2p openraft graph</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f8fafc;
      --panel: #ffffff;
      --ink: #0f172a;
      --muted: #64748b;
      --line: #cbd5e1;
      --accent: #0f766e;
      --danger: #b91c1c;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--ink);
      font: 14px/1.45 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    header {{
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      padding: 18px 22px;
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }}
    h1 {{
      margin: 0;
      font-size: 18px;
      font-weight: 650;
      letter-spacing: 0;
    }}
    .meta {{
      margin-top: 4px;
      color: var(--muted);
      font-size: 13px;
    }}
    form {{
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;
    }}
    select,
    a {{
      min-height: 34px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      color: var(--ink);
      padding: 6px 10px;
      text-decoration: none;
      font: inherit;
    }}
    a.primary {{
      border-color: var(--accent);
      color: var(--accent);
      font-weight: 600;
    }}
    main {{
      padding: 18px;
    }}
    .graph {{
      width: 100%;
      min-height: calc(100vh - 118px);
      border: 1px solid var(--line);
      border-radius: 8px;
      background: #fff;
      overflow: auto;
    }}
    .graph img {{
      display: block;
      min-width: 760px;
      max-width: none;
      width: 100%;
      height: auto;
    }}
    .error {{
      margin: 0 0 12px;
      color: var(--danger);
      font-weight: 600;
    }}
    @media (max-width: 720px) {{
      header {{
        align-items: stretch;
        flex-direction: column;
      }}
      main {{
        padding: 10px;
      }}
      .graph {{
        min-height: calc(100vh - 178px);
      }}
    }}
  </style>
</head>
<body>
  <header>
    <div>
      <h1>libp2p / openraft graph</h1>
      <div class="meta">local peer_id: {} | refresh: 5s</div>
    </div>
    <form method="get" action="/graph">
      <select name="group_id" aria-label="Raft group" onchange="this.form.submit()">{}</select>
      <a class="primary" href="/graph.svg?group_id={}">SVG</a>
      <a href="/graph.dot?group_id={}">DOT</a>
      <a href="/cluster?group_id={}">JSON</a>
    </form>
  </header>
  <main>
    {}
    <div class="graph">
      <img src="/graph.svg?group_id={}" alt="libp2p and openraft topology">
    </div>
  </main>
</body>
</html>"#,
    html_escape(&snapshot.self_peer_id),
    group_options,
    url_escape(selected_param),
    url_escape(selected_param),
    url_escape(selected_param),
    status,
    url_escape(selected_param),
  )
}

fn html_escape(value: &str) -> String {
  value
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

fn url_escape(value: &str) -> String {
  value
    .bytes()
    .flat_map(|byte| match byte {
      b'A' ..= b'Z' | b'a' ..= b'z' | b'0' ..= b'9' | b'-' | b'_' | b'.' | b'~' => {
        vec![byte as char]
      }
      _ => format!("%{byte:02X}").chars().collect(),
    })
    .collect()
}
