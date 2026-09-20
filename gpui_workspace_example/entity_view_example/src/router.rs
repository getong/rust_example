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

/// 路由模板、页面注册和导航的公共错误，不暴露底层匹配器类型。
#[derive(Debug, PartialEq, Eq)]
pub enum RouterError {
  /// 具体路径未匹配任何已注册模板。
  UnknownPath,
  /// 模板匹配成功，但页面尚未注册或已经关闭。
  ClosedPath,
  /// 具体路径已有页面实例。
  DuplicatePath,
  /// 模板语法非法；reason 仅用于诊断，不应依赖其文本判断错误类别。
  InvalidPattern { reason: String },
  /// 模板与已注册的 with 模板冲突（包括重复注册）。
  ConflictingPattern { with: String },
}

impl fmt::Display for RouterError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::UnknownPath => f.write_str("path does not match a registered route"),
      Self::ClosedPath => f.write_str("page is not open for this path"),
      Self::DuplicatePath => f.write_str("page path is already registered"),
      Self::InvalidPattern { reason } => write!(f, "invalid route pattern: {reason}"),
      Self::ConflictingPattern { with } => write!(f, "route pattern conflicts with {with}"),
    }
  }
}

impl Error for RouterError {}

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
  ///
  /// # Errors
  /// 非法模板返回 [`RouterError::InvalidPattern`]；重复或冲突模板返回
  /// [`RouterError::ConflictingPattern`]。
  pub fn register_route(&mut self, pattern: &str) -> Result<(), RouterError> {
    self
      .routes
      .insert(pattern, ())
      .map_err(|error| match error {
        matchit::InsertError::Conflict { with } => RouterError::ConflictingPattern { with },
        error => RouterError::InvalidPattern {
          reason: error.to_string(),
        },
      })
  }

  /// 在已定义的路由下添加页面实例，不覆盖已有页面及其局部状态。
  ///
  /// # Errors
  /// 未匹配模板返回 [`RouterError::UnknownPath`]；页面已存在返回
  /// [`RouterError::DuplicatePath`]。失败时保留已有页面和当前导航。
  pub fn register(&mut self, path: &str, view: AnyView) -> Result<(), RouterError> {
    self.routes.at(path).map_err(|_| RouterError::UnknownPath)?;
    match self.pages.entry(path.to_owned()) {
      std::collections::hash_map::Entry::Vacant(entry) => {
        entry.insert(view);
        Ok(())
      }
      std::collections::hash_map::Entry::Occupied(_) => Err(RouterError::DuplicatePath),
    }
  }

  /// 读取当前具体路径中的参数，例如 `/counter/{id}` 的 `id`。
  pub fn param(&self, name: &str) -> Option<&str> {
    self.routes.at(self.pathname()?).ok()?.params.get(name)
  }

  /// 导航到已打开的页面；重复导航到当前路径不改变状态。
  ///
  /// # Errors
  /// 未匹配模板返回 [`RouterError::UnknownPath`]；匹配模板但页面尚未注册或
  /// 已关闭返回 [`RouterError::ClosedPath`]。失败时保留当前页面和参数。
  pub fn navigate(&mut self, path: &str, cx: &mut Context<Self>) -> Result<(), RouterError> {
    self.routes.at(path).map_err(|_| RouterError::UnknownPath)?;
    let view = self.pages.get(path).ok_or(RouterError::ClosedPath)?.clone();
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
