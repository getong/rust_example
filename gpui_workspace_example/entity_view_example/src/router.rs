//! 路径匹配使用 matchit，页面承载使用与 Kit 同版本的 NavStack。
use std::{collections::HashMap, error::Error, fmt};

use gpui_kit::{
  AnyView, AppContext, Context, Entity, SharedString,
  base::{NavMotion, NavStackState},
};

pub struct TabRouter {
  routes: matchit::Router<()>,
  pages: HashMap<String, AnyView>,
  pathname: Option<SharedString>,
  stack: Entity<NavStackState>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RegisterError {
  UnknownRoute,
  DuplicatePath,
}

impl fmt::Display for RegisterError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::UnknownRoute => "page path does not match a registered route",
      Self::DuplicatePath => "page path is already registered",
    })
  }
}

impl Error for RegisterError {}

impl TabRouter {
  pub fn new(cx: &mut Context<Self>) -> Self {
    Self {
      routes: matchit::Router::new(),
      pages: HashMap::new(),
      pathname: None,
      stack: cx.new(|_| NavStackState::new()),
    }
  }

  pub fn pathname(&self) -> Option<&str> {
    self.pathname.as_deref()
  }

  pub fn stack(&self) -> &Entity<NavStackState> {
    &self.stack
  }

  /// 模板可在运行时添加；注册模板不会创建页面或改变当前导航。
  pub fn register_route(&mut self, pattern: &str) -> Result<(), matchit::InsertError> {
    self.routes.insert(pattern, ())
  }

  /// 在已定义的路由下添加页面实例，不覆盖已有页面及其局部状态。
  pub fn register(&mut self, path: &str, view: AnyView) -> Result<(), RegisterError> {
    self
      .routes
      .at(path)
      .map_err(|_| RegisterError::UnknownRoute)?;
    match self.pages.entry(path.to_owned()) {
      std::collections::hash_map::Entry::Vacant(entry) => {
        entry.insert(view);
        Ok(())
      }
      std::collections::hash_map::Entry::Occupied(_) => Err(RegisterError::DuplicatePath),
    }
  }

  /// 读取当前具体路径中的参数，例如 `/counter/{id}` 的 `id`。
  pub fn param(&self, name: &str) -> Option<&str> {
    self.routes.at(self.pathname()?).ok()?.params.get(name)
  }

  /// 未注册或已关闭的路径返回错误，保留当前页面。
  pub fn navigate(
    &mut self,
    path: &str,
    cx: &mut Context<Self>,
  ) -> Result<(), matchit::MatchError> {
    self.routes.at(path)?;
    let view = self
      .pages
      .get(path)
      .ok_or(matchit::MatchError::NotFound)?
      .clone();
    if self.pathname() == Some(path) {
      return Ok(());
    }
    self.stack.update(cx, |stack, cx| {
      // 标签是平级页面，使用 replace，避免历史持有已关闭的标签。
      stack.replace(view, NavMotion::Immediate, cx);
    });
    self.pathname = Some(path.to_owned().into());
    cx.notify();
    Ok(())
  }

  pub fn unregister(&mut self, path: &str, cx: &mut Context<Self>) {
    // 只移除具体页面，保留模板供同类页面继续使用和后续动态添加。
    self.pages.remove(path);
    if self.pathname() == Some(path) {
      // 同时释放 current 和过渡中的 outgoing 视图。
      self.stack.update(cx, NavStackState::clear);
      self.pathname = None;
      cx.notify();
    }
  }
}
