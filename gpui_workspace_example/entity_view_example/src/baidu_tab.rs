use std::{sync::Arc, time::Duration};

use anyhow::{Context as _, Result, bail, ensure};
use futures::{
  AsyncReadExt as _,
  future::{Either, select},
};
use gpui_kit::{
  component::{
    ActiveTheme, Disableable,
    button::Button,
    h_flex,
    input::{InputEvent, InputState, NumberInput},
    label::Label,
    pagination::Pagination,
    v_flex,
  },
  http_client::{AsyncBody, HttpClient, HttpRequestExt, RedirectPolicy, Request},
  *,
};
use serde::Deserialize;

use crate::scroll_panel::ScrollPanel;

pub(crate) const BOARD_URL: &str = "https://top.baidu.com/board?platform=pc&sa=pcindex_entry";
const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const MAX_BODY: u64 = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Envelope {
  data: BoardData,
}

#[derive(Debug, Deserialize)]
struct BoardData {
  cards: Vec<Board>,
}

#[derive(Debug, Deserialize)]
struct Board {
  text: String,
  content: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
  word: String,
  #[serde(default)]
  desc: String,
  #[serde(default)]
  hot_score: String,
  #[serde(default)]
  url: String,
}

fn decode_board(body: &str) -> Result<Vec<Board>> {
  let json = if body.trim_start().starts_with('{') {
    body
  } else {
    body
      .split_once("<!--s-data:")
      .context("页面中未找到榜单 JSON，页面结构可能已改变")?
      .1
      .split_once("-->")
      .context("榜单 JSON 标记不完整")?
      .0
  };
  let envelope: Envelope = serde_json::from_str(json).context("榜单 JSON 解码失败")?;
  let boards: Vec<_> = envelope
    .data
    .cards
    .into_iter()
    .filter(|board| !board.content.is_empty())
    .collect();
  ensure!(!boards.is_empty(), "返回的榜单没有内容");
  Ok(boards)
}

async fn fetch_board(client: Arc<dyn HttpClient>) -> Result<Vec<Board>> {
  let request = Request::builder()
    .uri(BOARD_URL)
    .follow_redirects(RedirectPolicy::FollowLimit(3))
    .timeout(Duration::from_secs(20))
    .body(AsyncBody::empty())?;
  let response = client.send(request).await.context("请求百度热榜失败")?;
  ensure!(
    response.status().is_success(),
    "服务器返回 HTTP {}",
    response.status()
  );
  let mut body = Vec::new();
  response
    .into_body()
    .take(MAX_BODY + 1)
    .read_to_end(&mut body)
    .await
    .context("读取榜单响应失败")?;
  ensure!(body.len() as u64 <= MAX_BODY, "榜单响应过大");
  decode_board(std::str::from_utf8(&body).context("响应不是 UTF-8 文本")?)
}

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 500;

#[derive(Debug)]
struct Paging {
  page: usize,
  size: usize,
}

impl Default for Paging {
  fn default() -> Self {
    Self {
      page: 1,
      size: DEFAULT_PAGE_SIZE,
    }
  }
}

impl Paging {
  fn pages(&self, total: usize) -> usize {
    total.div_ceil(self.size).max(1)
  }
  fn clamp(&mut self, total: usize) {
    self.page = self.page.clamp(1, self.pages(total));
  }
  fn range(&self, total: usize) -> std::ops::Range<usize> {
    let start = ((self.page - 1) * self.size).min(total);
    start .. (start + self.size).min(total)
  }
  fn set_size(&mut self, size: usize) -> bool {
    if !(1 ..= MAX_PAGE_SIZE).contains(&size) || size == self.size {
      return false;
    }
    self.size = size;
    self.page = 1;
    true
  }
}

pub(crate) struct BaiduTab {
  boards: Vec<Board>,
  paging: Paging,
  size_input: Option<Entity<InputState>>,
  size_subscription: Option<Subscription>,
  loading: bool,
  error: Option<String>,
  updated: Option<String>,
  scroll: ScrollHandle,
  request: Option<Task<()>>,
  timer: Option<Task<()>>,
}

impl BaiduTab {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    let mut view = Self {
      boards: Vec::new(),
      paging: Paging::default(),
      size_input: None,
      size_subscription: None,
      loading: false,
      error: None,
      updated: None,
      scroll: ScrollHandle::new(),
      request: None,
      timer: None,
    };
    view.refresh(cx);
    // Context::spawn 提供 WeakEntity；任务不会保活已关闭的标签。
    view.timer = Some(cx.spawn(async move |view, cx| {
      loop {
        cx.background_executor().timer(REFRESH_INTERVAL).await;
        if view.update(cx, |view, cx| view.refresh(cx)).is_err() {
          break;
        }
      }
    }));
    view
  }

  fn total_items(&self) -> usize {
    self.boards.iter().map(|board| board.content.len()).sum()
  }

  fn select_page(&mut self, page: usize, cx: &mut Context<Self>) {
    self.paging.page = page;
    self.paging.clamp(self.total_items());
    self.scroll.set_offset(point(px(0.), px(0.)));
    cx.notify();
  }

  fn size_input(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
    if let Some(input) = &self.size_input {
      return input.clone();
    }
    let input = cx.new(|cx| {
      InputState::new(window, cx)
        .default_value(self.paging.size.to_string())
        .min(1.)
        .max(MAX_PAGE_SIZE as f64)
        .step(1.)
    });
    self.size_subscription = Some(cx.subscribe(&input, |view, input, event, cx| {
      if let InputEvent::Change = event
        && let Ok(size) = input.read(cx).value().parse::<usize>()
        && view.paging.set_size(size)
      {
        view.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
      }
    }));
    self.size_input = Some(input.clone());
    input
  }

  fn refresh(&mut self, cx: &mut Context<Self>) {
    if self.loading {
      return;
    }
    self.loading = true;
    self.error = None;
    let client = cx.http_client();
    let executor = cx.background_executor().clone();
    self.request = Some(cx.spawn(async move |view, cx| {
      let job = executor.clone().spawn(async move {
        match select(
          Box::pin(fetch_board(client)),
          Box::pin(executor.timer(Duration::from_secs(20))),
        )
        .await
        {
          Either::Left((result, _)) => result,
          Either::Right(_) => bail!("请求超时，请稍后重试"),
        }
      });
      let result = job.await;
      let _ = view.update(cx, |view, cx| {
        view.loading = false;
        match result {
          Ok(boards) => {
            view.boards = boards;
            let old_page = view.paging.page;
            view.paging.clamp(view.total_items());
            if view.paging.page != old_page {
              view.scroll.set_offset(point(px(0.), px(0.)));
            }
            view.updated = Some(chrono::Local::now().format("%H:%M:%S").to_string());
          }
          Err(error) => view.error = Some(format!("{error:#}")),
        }
        cx.notify();
      });
    }));
    cx.notify();
  }

}

impl Render for BaiduTab {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let size_input = self.size_input(window, cx);
    let total = self.total_items();
    let range = self.paging.range(total);
    let mut offset = 0;
    v_flex()
      .size_full()
      .p_4()
      .gap_3()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        h_flex()
          .gap_3()
          .child(Label::new("百度热榜").text_2xl())
          .child(
            Button::new("baidu-refresh")
              .label(if self.loading {
                "正在刷新…"
              } else {
                "立即刷新"
              })
              .disabled(self.loading)
              .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
          ),
      )
      .child(Label::new(format!(
        "每 5 分钟自动刷新 · {}",
        self
          .updated
          .as_ref()
          .map_or("尚未更新".to_string(), |time| format!(
            "上次更新 {time}"
          ))
      )))
      .children(self.error.as_ref().map(|error| {
        Label::new(format!(
          "刷新失败：{error}。已有内容保留，可点击立即刷新重试。"
        ))
        .text_color(cx.theme().danger)
      }))
      .child(
        ScrollPanel::new("baidu-board-scroll", &self.scroll)
          .children((self.boards.is_empty()).then(|| {
            Label::new(if self.loading {
              "正在获取榜单…"
            } else {
              "暂时没有榜单内容"
            })
            .p_4()
          }))
          .children(
            self
              .boards
              .iter()
              .enumerate()
              .filter_map(|(board_index, board)| {
                let start = range.start.saturating_sub(offset).min(board.content.len());
                let end = range.end.saturating_sub(offset).min(board.content.len());
                offset += board.content.len();
                if start == end {
                  return None;
                }
                Some(
                  v_flex()
                    .gap_2()
                    .p_4()
                    .child(
                      Label::new(format!("{} · {} 条", board.text, board.content.len())).text_xl(),
                    )
                    .children(
                      board
                        .content
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(end - start)
                        .map(|(index, entry)| {
                          let url = entry.url.clone();
                          v_flex()
                            .gap_1()
                            .py_3()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                              h_flex()
                                .gap_3()
                                .child(Label::new(format!("{}", index + 1)).w_8())
                                .child(
                                  Button::new(("baidu-entry", board_index * 1000 + index))
                                    .flex_1()
                                    .min_w_0()
                                    .tooltip(entry.word.clone())
                                    .label(entry.word.clone())
                                    .on_click(move |_, _, cx| {
                                      if url.starts_with("https://") || url.starts_with("http://") {
                                        cx.open_url(&url);
                                      }
                                    }),
                                )
                                .child(
                                  Label::new(format!("热度 {}", entry.hot_score))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                                ),
                            )
                            .children((!entry.desc.is_empty()).then(|| {
                              Label::new(entry.desc.clone())
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                            }))
                        }),
                    ),
                )
              }),
          ),
      )
      .child(
        h_flex()
          .flex_wrap()
          .gap_3()
          .flex_shrink_0()
          .child(Label::new(format!(
            "共 {total} 条 · 显示 {}–{} 条",
            if total == 0 { 0 } else { range.start + 1 },
            range.end
          )))
          .child(Label::new("每页"))
          .child(NumberInput::new(&size_input).w_24())
          .child(Label::new("条（1–500）"))
          .child(
            Pagination::new("baidu-pagination")
              .total_pages(self.paging.pages(total))
              .current_page(self.paging.page)
              .disabled(total == 0)
              .on_click(cx.listener(|view, page: &usize, _, cx| view.select_page(*page, cx))),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use std::{
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
  };

  use gpui_kit::{
    AppContext, BorrowAppContext, TestAppContext,
    http_client::{AsyncBody, FakeHttpClient, Response},
  };

  use super::{BOARD_URL, BaiduTab, REFRESH_INTERVAL, decode_board};

  const JSON: &str = r#"{"data":{"cards":[{"text":"热搜榜","content":[{"word":"示例热搜","desc":"内容包含中文与 > 符号","hotScore":"12345","url":"https://www.baidu.com/"}]},{"text":"电影榜","content":[{"word":"示例电影"}]}]}}"#;

  #[test]
  fn decodes_embedded_json_and_rejects_invalid_pages() {
    let page = format!("<html><!--s-data:{JSON}--><main>page</main></html>");
    let boards = decode_board(&page).unwrap();
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].content[0].word, "示例热搜");
    assert_eq!(boards[0].content[0].hot_score, "12345");
    assert!(boards[1].content[0].desc.is_empty());
    assert_eq!(decode_board(JSON).unwrap().len(), 2);
    assert!(decode_board("<html>访问验证</html>").is_err());
    assert!(decode_board("<!--s-data:{broken}-->").is_err());
    assert!(decode_board(r#"{"data":{"cards":[]}}"#).is_err());
  }

  #[gpui_kit::test]
  fn refreshes_every_five_minutes_retains_data_on_failure_and_stops_on_close(
    cx: &mut TestAppContext,
  ) {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    cx.update(|cx| {
      cx.set_http_client(FakeHttpClient::create(move |request| {
        assert_eq!(request.uri().to_string(), BOARD_URL);
        let call = count.fetch_add(1, Ordering::SeqCst);
        async move {
          Ok(
            Response::builder()
              .status(if call == 1 { 503 } else { 200 })
              .body(AsyncBody::from(JSON))
              .unwrap(),
          )
        }
      }))
    });
    let tab = cx.new(BaiduTab::new);
    tab.update(cx, |tab, cx| tab.refresh(cx)); // Loading requests are not duplicated.
    cx.run_until_parked();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tab.read_with(cx, |tab, _| {
      assert_eq!(tab.boards.len(), 2);
      assert!(!tab.loading);
    });
    cx.background_executor
      .advance_clock(Duration::from_secs(299));
    cx.run_until_parked();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    cx.background_executor.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tab.read_with(cx, |tab, _| {
      assert_eq!(tab.boards.len(), 2);
      assert!(tab.error.as_ref().unwrap().contains("503"));
    });
    tab.update(cx, |tab, cx| tab.refresh(cx));
    cx.run_until_parked();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    tab.read_with(cx, |tab, _| assert!(tab.error.is_none()));
    drop(tab);
    cx.run_until_parked();
    cx.background_executor.advance_clock(REFRESH_INTERVAL);
    cx.run_until_parked();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
  }

  #[gpui_kit::test]
  fn request_timeout_releases_loading_state(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_http_client(FakeHttpClient::create(|_| futures::future::pending())));
    let tab = cx.new(BaiduTab::new);
    cx.run_until_parked();
    cx.background_executor
      .advance_clock(Duration::from_secs(20));
    cx.run_until_parked();
    tab.read_with(cx, |tab, _| {
      assert!(!tab.loading);
      assert!(tab.error.as_ref().unwrap().contains("超时"));
    });
  }
  #[test]
  #[ignore = "Live network verification; run explicitly"]
  fn live_baidu_response_decodes() {
    let client = reqwest_client::ReqwestClient::user_agent("component-gallery").unwrap();
    let boards = futures::executor::block_on(super::fetch_board(Arc::new(client))).unwrap();
    assert!(boards.iter().any(|board| board.text.contains("热搜")));
    assert!(boards.iter().all(|board| !board.content.is_empty()));
    eprintln!(
      "Decoded {} boards, {} entries",
      boards.len(),
      boards.iter().map(|b| b.content.len()).sum::<usize>()
    );
  }
  #[test]
  fn pagination_handles_boundaries_and_invalid_sizes() {
    let mut paging = super::Paging::default();
    assert_eq!(paging.range(45), 0 .. 20);
    assert_eq!(paging.pages(40), 2);
    paging.page = 3;
    assert_eq!(paging.range(45), 40 .. 45);
    paging.clamp(12);
    assert_eq!(paging.page, 1);
    assert!(!paging.set_size(0));
    assert!(!paging.set_size(501));
    assert_eq!(paging.size, 20);
    assert!(paging.set_size(7));
    assert_eq!(paging.range(45), 0 .. 7);
    assert_eq!(paging.pages(45), 7);
    paging.clamp(0);
    assert_eq!(paging.range(0), 0 .. 0);
  }

  #[gpui_kit::test]
  fn pagination_clicks_size_changes_and_refresh_clamping(cx: &mut TestAppContext) {
    use gpui_kit::{component::Root, point, px, size, test::TestWindowExt};
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let board = |name: &str, count: usize| {
      serde_json::json!({"text":name,
      "content":(0..count).map(|i|serde_json::json!({"word":format!("{name}-{i}")})).collect::<Vec<_>>()})
    };
    let large = serde_json::json!({"data":{"cards":[board("A",15),board("B",30)]}}).to_string();
    cx.update(|cx| {
      gpui_kit::init(cx);
      cx.set_http_client(FakeHttpClient::create(move |_| {
        let body = if count.fetch_add(1, Ordering::SeqCst) == 0 {
          large.clone()
        } else {
          JSON.to_string()
        };
        async move {
          Ok(
            Response::builder()
              .status(200)
              .body(AsyncBody::from(body))
              .unwrap(),
          )
        }
      }));
    });
    let tab = cx.new(BaiduTab::new);
    let window = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
      Root::new(tab.clone(), window, cx)
    });
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| {
      window.render_frame(cx);
      assert_eq!(tab.read(cx).paging.range(45), 0 .. 20);
      window.within("baidu-pagination").click(2usize, cx);
      assert_eq!(tab.read(cx).paging.page, 2);
      assert_eq!(window.find(("baidu-entry", 1005usize)).label(), Some("B-5"));
      let input = tab.read(cx).size_input.clone().unwrap();
      input.update(cx, |input, cx| {
        input.set_value("7", window, cx);
        // Programmatic set_value is silent; model the user's Change event.
        cx.emit(super::InputEvent::Change);
      });
    })
    .unwrap();
    cx.run_until_parked();
    tab.read_with(cx, |tab, _| {
      assert_eq!(tab.paging.size, 7);
      assert_eq!(tab.paging.page, 1);
      assert_eq!(tab.scroll.offset(), point(px(0.), px(0.)));
    });
    tab.update(cx, |tab, cx| {
      tab.select_page(7, cx);
      tab.refresh(cx);
    });
    cx.run_until_parked();
    tab.read_with(cx, |tab, _| {
      assert_eq!(tab.total_items(), 2);
      assert_eq!(tab.paging.page, 1);
      assert_eq!(tab.paging.size, 7);
    });
  }
}
