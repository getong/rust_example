use clap::Parser;
use gpui_kit::{
  AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
  StatefulInteractiveElement, Styled, Task, Window, WindowBounds, WindowOptions,
  component::{ActiveTheme, Disableable, Root, Selectable, button::*, input::*},
  div, px, size,
};
use olpc_rocksdb_viewer::{
  Page, Query, read_page,
  records::{Records, label},
};

#[derive(Parser)]
#[command(about = "GPUI RocksDB 数据浏览器（HTTP 只读连接）")]
struct Args {
  /// Node HTTP base URL.
  #[arg(long, default_value = "http://127.0.0.1:3001")]
  url: String,
  /// Raft group; empty uses the node default.
  #[arg(long, default_value = "")]
  group: String,
  /// Column family; defaults to sm_data when available.
  #[arg(long)]
  cf: Option<String>,
  /// UTF-8 key prefix.
  #[arg(long, default_value = "")]
  prefix: String,
}

struct Viewer {
  url: Entity<InputState>,
  group: Entity<InputState>,
  prefix: Entity<InputState>,
  query: Query,
  page: Option<Page>,
  page_number: usize,
  records: Records,
  selected_row: Option<usize>,
  loading: bool,
  error: Option<String>,
  request: Option<Task<()>>,
}

impl Viewer {
  fn new(args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let url_input = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("节点 URL，例如 http://127.0.0.1:3001")
        .default_value(args.url.clone())
    });
    let group_input = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("Raft group（留空使用节点默认组）")
        .default_value(args.group.clone())
    });
    let prefix_input = cx.new(|cx| {
      InputState::new(window, cx)
        .placeholder("按 key 前缀过滤（UTF-8，可留空）")
        .default_value(args.prefix.clone())
    });
    let mut view = Self {
      url: url_input,
      group: group_input,
      prefix: prefix_input,
      query: Query {
        url: args.url,
        group_id: args.group,
        column_family: args.cf,
        prefix: args.prefix,
        after: None,
      },
      page: None,
      page_number: 1,
      records: Records::default(),
      selected_row: None,
      loading: false,
      error: None,
      request: None,
    };
    if !view.query.url.is_empty() {
      view.load(view.query.clone(), 1, cx);
    }
    view
  }

  fn first_page(&mut self, cx: &mut Context<Self>) {
    let url = self.url.read(cx).value().trim().to_owned();
    let group_id = self.group.read(cx).value().trim().to_owned();
    let column_family = if url == self.query.url && group_id == self.query.group_id {
      self.query.column_family.clone()
    } else {
      None
    };
    let query = Query {
      url,
      group_id,
      column_family,
      prefix: self.prefix.read(cx).value().to_string(),
      after: None,
    };
    self.load(query, 1, cx);
  }

  fn load(&mut self, mut query: Query, page_number: usize, cx: &mut Context<Self>) {
    if self.loading {
      return;
    }
    self.loading = true;
    self.error = None;
    let work = query.clone();
    let task = cx
      .background_executor()
      .spawn(async move { read_page(&work) });
    self.request = Some(cx.spawn(async move |view, cx| {
      let result = task.await;
      let _ = view.update(cx, |view, cx| {
        view.loading = false;
        match result {
          Ok(page) => {
            query.column_family = Some(page.column_family.clone());
            view.query = query;
            view.records = Records::from_rows(&page.rows);
            view.selected_row = (!view.records.rows.is_empty()).then_some(0);
            view.page = Some(page);
            view.page_number = page_number;
          }
          Err(error) => view.error = Some(format!("{error:#}")),
        }
        cx.notify();
      });
    }));
    cx.notify();
  }
}

impl Render for Viewer {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let mut families = div().flex().gap_2();
    if let Some(page) = &self.page {
      for (index, name) in page.column_families.iter().enumerate() {
        let family = name.clone();
        families = families.child(
          Button::new(("cf", index))
            .label(name.clone())
            .selected(*name == page.column_family)
            .disabled(self.loading)
            .on_click(cx.listener(move |view, _, _, cx| {
              let mut query = view.query.clone();
              query.column_family = Some(family.clone());
              query.after = None;
              view.load(query, 1, cx);
            })),
        );
      }
    }
    let next = self.page.as_ref().and_then(|page| page.next.clone());
    let border = theme.border;
    let muted = theme.muted_foreground;
    let selected_color = theme.secondary;
    let width = 420. + self.records.columns.len() as f32 * 190.;
    let cell = |text: String, width: f32| {
      div()
        .w(px(width))
        .flex_shrink_0()
        .px_3()
        .py_2()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .border_r_1()
        .border_color(border)
        .child(text)
    };
    let mut header = div()
      .flex()
      .h(px(42.))
      .overflow_hidden()
      .flex_shrink_0()
      .bg(selected_color)
      .border_b_1()
      .border_color(border)
      .child(cell("序号".into(), 60.))
      .child(cell("记录标识".into(), 260.))
      .child(cell("数据类型".into(), 100.));
    for column in &self.records.columns {
      header = header.child(cell(label(column), 190.));
    }
    let mut body = div()
      .id("record-rows")
      .flex_1()
      .min_h_0()
      .overflow_y_scroll();
    for (index, record) in self.records.rows.iter().enumerate() {
      let mut row = div()
        .id(("record", index))
        .flex()
        .h(px(42.))
        .cursor_pointer()
        .border_b_1()
        .border_color(border)
        .on_click(cx.listener(move |view, _, _, cx| {
          view.selected_row = Some(index);
          cx.notify();
        }));
      if self.selected_row == Some(index) {
        row = row.bg(selected_color);
      }
      row = row
        .child(cell(
          ((self.page_number - 1) * 100 + index + 1).to_string(),
          60.,
        ))
        .child(cell(record.key.clone(), 260.))
        .child(cell(record.kind.clone(), 100.));
      for column in &self.records.columns {
        row = row.child(cell(
          record
            .fields
            .get(column)
            .cloned()
            .unwrap_or_else(|| "—".into()),
          190.,
        ));
      }
      body = body.child(row);
    }
    if self.records.rows.is_empty() {
      body = body.child(div().p_8().text_color(muted).child(if self.page.is_some() {
        "没有匹配的数据，请调整 group、列族或前缀。"
      } else {
        "连接节点后，业务字段将按列展示。"
      }));
    }
    let rows = div()
      .id("record-table")
      .flex_1()
      .min_h_0()
      .overflow_x_scroll()
      .border_1()
      .border_color(border)
      .child(
        div()
          .w(px(width.max(700.)))
          .h_full()
          .flex()
          .flex_col()
          .child(header)
          .child(body),
      );
    let mut details = div().h(px(210.)).flex_shrink_0().flex().flex_col().gap_2();
    if let Some(record) = self
      .selected_row
      .and_then(|index| self.records.rows.get(index))
    {
      details = details.child(div().text_sm().child(format!(
        "字段明细 · {} · {} 个字段",
        record.key,
        record.fields.len()
      )));
      let mut fields = div()
        .id("record-details")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .border_1()
        .border_color(border);
      if let Some(notice) = &record.notice {
        fields = fields.child(div().p_3().text_color(muted).child(notice.clone()));
      }
      for (path, value) in &record.fields {
        fields = fields.child(
          div()
            .flex()
            .border_b_1()
            .border_color(border)
            .child(
              div()
                .w(px(280.))
                .flex_shrink_0()
                .p_2()
                .text_color(muted)
                .child(label(path)),
            )
            .child(div().flex_1().min_w_0().p_2().child(value.clone())),
        );
      }
      details = details.child(fields);
    }
    let status = if self.loading {
      "正在读取…".to_owned()
    } else if let Some(page) = &self.page {
      format!(
        "{}  /  {}  /  {}  · 第 {} 页 · {} 条",
        self.query.url,
        page.group_id,
        page.column_family,
        self.page_number,
        page.rows.len()
      )
    } else {
      "尚未连接节点".to_owned()
    };
    let mut content = div()
      .size_full()
      .flex()
      .flex_col()
      .gap_3()
      .p_5()
      .bg(theme.background)
      .text_color(theme.foreground)
      .child(div().text_xl().child("RocksDB 数据浏览器"))
      .child(
        div()
          .text_sm()
          .text_color(theme.muted_foreground)
          .child("每条记录一行 · 主表最多 24 个字段 · 横向滚动查看更多 · 点击记录查看更多字段明细"),
      )
      .child(
        div()
          .flex()
          .gap_2()
          .child(
            div()
              .flex_1()
              .child(Input::new(&self.url).disabled(self.loading)),
          )
          .child(
            Button::new("open")
              .primary()
              .label("连接 / 刷新")
              .disabled(self.loading)
              .on_click(cx.listener(|view, _, _, cx| view.first_page(cx))),
          ),
      )
      .child(Input::new(&self.group).disabled(self.loading))
      .child(Input::new(&self.prefix).disabled(self.loading))
      .child(families)
      .child(div().text_sm().child(status));
    if let Some(error) = &self.error {
      content = content.child(
        div()
          .p_3()
          .text_color(theme.danger)
          .child(format!("读取失败：{error}（下方保留上次成功读取的数据）")),
      );
    }
    content.child(rows).child(details).child(
      div()
        .flex()
        .gap_2()
        .child(
          Button::new("first")
            .label("首页")
            .disabled(self.loading || self.page.is_none())
            .on_click(cx.listener(|view, _, _, cx| {
              let mut query = view.query.clone();
              query.after = None;
              view.load(query, 1, cx);
            })),
        )
        .child(
          Button::new("next")
            .label("下一页")
            .disabled(self.loading || next.is_none())
            .on_click(cx.listener(move |view, _, _, cx| {
              if let Some(after) = &next {
                let mut query = view.query.clone();
                query.after = Some(after.clone());
                view.load(query, view.page_number + 1, cx);
              }
            })),
        ),
    )
  }
}

fn main() {
  let args = Args::parse();
  gpui_kit::application()
    .with_assets(gpui_kit::assets::Assets)
    .run(move |cx| {
      gpui_kit::init(cx);
      cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();
      let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
          None,
          size(px(1100.), px(760.)),
          cx,
        ))),
        window_min_size: Some(size(px(760.), px(650.))),
        ..Default::default()
      };
      if let Err(error) = cx.open_window(options, |window, cx| {
        window.set_window_title("OLPC · RocksDB");
        let viewer = cx.new(|cx| Viewer::new(args, window, cx));
        cx.new(|cx| Root::new(viewer, window, cx))
      }) {
        eprintln!("无法打开数据浏览器：{error:#}");
        cx.quit();
      }
      cx.activate(true);
    });
}
