use std::{cell::RefCell, rc::Rc};

use gpui_kit::*;
use gpui_mai_ui_client::document;

const WELCOME: &str = "<!doctype html><html><head><style>body{margin:0;min-height:100vh;display:\
                       grid;place-items:center;background:#101827;color:#cbd5e1;font:16px \
                       system-ui}main{text-align:center;padding:40px}h1{color:#\
                       f8fafc}p{line-height:1.9;color:#94a3b8}small{color:#64748b}</style></\
                       head><body><main><h1>从一句提示词开始</h1><p>输入需求 → 流式生成 → \
                       即时预览<br>选择任意历史版本，继续探索新的设计方向。</p><small>HTML/CSS \
                       视觉原型 · 非真实业务应用</small></main></body></html>";

pub struct Preview {
  html: Rc<String>,
  pub error: Option<String>,
  #[cfg(any(target_os = "macos", target_os = "windows"))]
  view: Option<wry::WebView>,
  #[cfg(any(target_os = "macos", target_os = "windows"))]
  loaded: Option<Rc<String>>,
  attempted: bool,
}

impl Preview {
  pub fn new() -> Rc<RefCell<Self>> {
    Rc::new(RefCell::new(Self {
      html: Rc::new(document::preview(WELCOME)),
      error: None,
      #[cfg(any(target_os = "macos", target_os = "windows"))]
      view: None,
      #[cfg(any(target_os = "macos", target_os = "windows"))]
      loaded: None,
      attempted: false,
    }))
  }

  pub fn set_document(&mut self, html: Option<&str>) {
    self.html = Rc::new(document::preview(html.unwrap_or(WELCOME)));
  }

  pub fn hide(&mut self) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(view) = &self.view
      && let Err(error) = view.set_visible(false).and_then(|_| view.focus_parent())
    {
      self.error = Some(format!("隐藏预览失败: {error}"));
    }
  }

  pub fn focus_editor(&self) -> anyhow::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(view) = &self.view {
      view.focus_parent()?;
    }
    Ok(())
  }

  #[cfg(any(target_os = "macos", target_os = "windows"))]
  fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window) -> anyhow::Result<()> {
    let bounds = wry::Rect {
      position: wry::dpi::LogicalPosition::new(
        f64::from(bounds.origin.x),
        f64::from(bounds.origin.y),
      )
      .into(),
      size: wry::dpi::LogicalSize::new(f64::from(bounds.size.width), f64::from(bounds.size.height))
        .into(),
    };
    if self.view.is_none() && !self.attempted {
      self.attempted = true;
      self.view = Some(
        wry::WebViewBuilder::new()
          .with_html(self.html.as_str())
          .with_bounds(bounds)
          .with_focused(false)
          .with_devtools(false)
          .with_autoplay(false)
          .with_navigation_handler(|url| {
            url == "about:blank"
              || url == "about:srcdoc"
              || url.starts_with("about:blank#")
              || url.starts_with("about:srcdoc#")
          })
          .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
          .with_download_started_handler(|_, _| false)
          .build_as_child(&*window)?,
      );
      self.focus_editor()?;
      self.loaded = Some(self.html.clone());
    }
    if let Some(view) = &self.view {
      view.set_bounds(bounds)?;
      view.set_visible(true)?;
      if self
        .loaded
        .as_ref()
        .is_none_or(|loaded| !Rc::ptr_eq(loaded, &self.html))
      {
        view.load_html(self.html.as_str())?;
        self.loaded = Some(self.html.clone());
      }
    }
    Ok(())
  }

  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  fn paint(&mut self, _: Bounds<Pixels>, _: &mut Window) -> anyhow::Result<()> {
    if !self.attempted {
      self.attempted = true;
      anyhow::bail!("当前内嵌预览支持 macOS / Windows；其他平台可查看源码并导出 HTML");
    }
    Ok(())
  }
}

pub fn element(preview: Rc<RefCell<Preview>>) -> impl IntoElement {
  canvas(
    |_, _, _| (),
    move |bounds, _, window, cx| {
      let mut preview = preview.borrow_mut();
      if preview.error.is_none()
        && let Err(error) = preview.paint(bounds, window)
      {
        preview.error = Some(format!("内嵌预览不可用: {error:#}；仍可查看源码、导出页面"));
        preview.hide();
        cx.refresh_windows();
      }
    },
  )
  .size_full()
}
