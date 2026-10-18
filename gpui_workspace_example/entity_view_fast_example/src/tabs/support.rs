//! Native component demonstrations adapted from gpui-kit 0.6.4 (Apache-2.0).
use std::rc::Rc;

use gpui_kit::{
  component::{
    ActiveTheme, Sizable as _, Size as ComponentSize, StyledExt as _,
    button::Button,
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    menu::PopupMenu,
    popover::Popover,
    v_flex,
  },
  prelude::FluentBuilder as _,
  *,
};
use serde::Deserialize;

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = component_tabs, no_json)]
pub(crate) struct ChangeStorySize(pub ComponentSize);
actions!(component_tabs, [TestAction]);

/// Enable remote image examples for the real application; tests retain their fake HTTP client.
pub fn init_http(cx: &mut App) {
  if let Ok(client) = reqwest_client::ReqwestClient::user_agent("component-gallery") {
    cx.set_http_client(std::sync::Arc::new(client));
  }
}
#[derive(IntoElement)]
pub(crate) struct StorySection {
  base: Div,
  title: SharedString,
  description: Option<SharedString>,
  sub_title: Vec<AnyElement>,
  children: Vec<AnyElement>,
}

impl StorySection {
  pub fn description(mut self, description: impl Into<SharedString>) -> Self {
    self.description = Some(description.into());
    self
  }

  pub fn sub_title(mut self, sub_title: impl IntoElement) -> Self {
    self.sub_title.push(sub_title.into_any_element());
    self
  }

  #[allow(unused)]
  fn max_w_md(mut self) -> Self {
    self.base = self.base.max_w(rems(48.));
    self
  }

  #[allow(unused)]
  fn max_w_lg(mut self) -> Self {
    self.base = self.base.max_w(rems(64.));
    self
  }

  #[allow(unused)]
  fn max_w_xl(mut self) -> Self {
    self.base = self.base.max_w(rems(80.));
    self
  }

  #[allow(unused)]
  fn max_w_2xl(mut self) -> Self {
    self.base = self.base.max_w(rems(96.));
    self
  }
}

impl ParentElement for StorySection {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for StorySection {
  fn style(&mut self) -> &mut gpui_kit::StyleRefinement {
    self.base.style()
  }
}

impl RenderOnce for StorySection {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    GroupBox::new()
      .id(self.title.clone())
      .outline()
      .mb_6()
      .title(
        h_flex()
          .justify_between()
          .items_start()
          .w_full()
          .gap_4()
          .child(
            v_flex()
              .min_w_0()
              .flex_1()
              .gap_1()
              .child(div().font_medium().child(self.title))
              .when_some(self.description, |this, description| {
                this.child(
                  div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(description),
                )
              }),
          )
          .children(self.sub_title),
      )
      .content_style(
        StyleRefinement::default()
          .rounded(cx.theme().radius_lg)
          .overflow_x_hidden()
          .items_center()
          .justify_center(),
      )
      .child(self.base.children(self.children))
  }
}

pub(crate) fn section(title: impl Into<SharedString>) -> StorySection {
  StorySection {
    title: title.into(),
    description: None,
    sub_title: vec![],
    base: h_flex()
      .w_full()
      .flex_wrap()
      .justify_center()
      .items_center()
      .gap_4(),
    children: vec![],
  }
}

#[derive(IntoElement)]
pub(crate) struct StoryToolbar {
  base: Div,
  items: Vec<StoryToolbarItem>,
}

enum StoryToolbarItem {
  Button(Button),
  Dropdown {
    button: Button,
    builder: StoryMenuBuilder,
  },
}

impl StoryToolbar {
  pub(crate) fn child(mut self, button: Button) -> Self {
    self.items.push(StoryToolbarItem::Button(button));
    self
  }

  pub(crate) fn dropdown_child(
    mut self,
    button: Button,
    builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    self.items.push(StoryToolbarItem::Dropdown {
      button,
      builder: Rc::new(builder),
    });
    self
  }
}

type StoryMenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

#[derive(Default)]
struct StoryToolbarMenuState {
  menu: Option<Entity<PopupMenu>>,
}

#[derive(IntoElement)]
struct StoryToolbarMenu {
  id: SharedString,
  button: Button,
  builder: StoryMenuBuilder,
}

impl RenderOnce for StoryToolbarMenu {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state =
      window.use_keyed_state(self.id.clone(), cx, |_, _| StoryToolbarMenuState::default());
    let builder = self.builder;

    let popover_id = SharedString::from(format!("story-toolbar-popover-{}", self.id));

    Popover::new(popover_id)
      .appearance(false)
      .overlay_closable(false)
      .anchor(Anchor::TopRight)
      .trigger(self.button)
      .content(move |_, window, cx| {
        if let Some(menu) = state.read(cx).menu.clone() {
          return menu;
        }

        let builder = builder.clone();
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
          builder(menu, window, cx)
        });
        state.update(cx, |state, _| state.menu = Some(menu.clone()));
        menu.focus_handle(cx).focus(window, cx);

        let popover = cx.entity();
        window
          .subscribe(&menu, cx, {
            let state = state.clone();
            move |_, _: &DismissEvent, window, cx| {
              popover.update(cx, |popover, cx| popover.dismiss(window, cx));
              state.update(cx, |state, _| state.menu = None);
            }
          })
          .detach();

        menu
      })
  }
}

impl Styled for StoryToolbar {
  fn style(&mut self) -> &mut StyleRefinement {
    self.base.style()
  }
}

impl RenderOnce for StoryToolbar {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let last = self.items.len().saturating_sub(1);

    self
      .base
      .children(self.items.into_iter().enumerate().map(|(ix, item)| {
        // Join the buttons into one segmented control: square off the
        // inner corners, and let each button after the first sit on its
        // neighbour's border instead of drawing a second one.
        let joined = |button: Button| {
          button
            .outline()
            .small()
            .when(ix > 0, |this| {
              this.rounded_tl(px(0.)).rounded_bl(px(0.)).border_l_0()
            })
            .when(ix < last, |this| this.rounded_tr(px(0.)).rounded_br(px(0.)))
        };

        match item {
          StoryToolbarItem::Button(button) => joined(button).into_any_element(),
          StoryToolbarItem::Dropdown { button, builder } => StoryToolbarMenu {
            id: SharedString::from(format!("story-toolbar-menu-{ix}")),
            button: joined(button),
            builder,
          }
          .into_any_element(),
        }
      }))
  }
}

pub(crate) fn story_toolbar_group() -> StoryToolbar {
  StoryToolbar {
    base: h_flex().w_full().justify_end(),
    items: vec![],
  }
}

pub(crate) fn story_toolbar(size: ComponentSize) -> StoryToolbar {
  let label = match size {
    ComponentSize::XSmall => "XSmall",
    ComponentSize::Small => "Small",
    ComponentSize::Medium => "Medium",
    ComponentSize::Large => "Large",
    ComponentSize::Size(_) => "Custom",
  };

  story_toolbar_group().dropdown_child(
    Button::new("story-size").label(format!("Size: {label}")),
    move |menu, _, _| {
      menu
        .menu_with_check(
          "XSmall",
          size == ComponentSize::XSmall,
          Box::new(ChangeStorySize(ComponentSize::XSmall)),
        )
        .menu_with_check(
          "Small",
          size == ComponentSize::Small,
          Box::new(ChangeStorySize(ComponentSize::Small)),
        )
        .menu_with_check(
          "Medium",
          size == ComponentSize::Medium,
          Box::new(ChangeStorySize(ComponentSize::Medium)),
        )
        .menu_with_check(
          "Large",
          size == ComponentSize::Large,
          Box::new(ChangeStorySize(ComponentSize::Large)),
        )
    },
  )
}

pub fn update_theme(cx: &mut App, update: impl FnOnce(&mut gpui_kit::component::Theme)) {
  let theme = gpui_kit::component::Theme::global_mut(cx);
  update(theme);
  gpui_kit::component::Theme::sync_base(cx);
  cx.refresh_windows();
}
