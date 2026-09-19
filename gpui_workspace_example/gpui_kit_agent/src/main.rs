use gpui_kit::{
  component::{
    ActiveTheme as _, Root, Selectable as _, Sizable as _, StyledExt as _, TITLE_BAR_HEIGHT,
    TitleBar,
    accordion::Accordion,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
  },
  prelude::FluentBuilder as _,
  *,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
  Home,
  Explore,
  Profile,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawerAction {
  Overview,
  Notifications,
  Appearance,
  Security,
  About,
  Version,
  Help,
}

impl Tab {
  fn title(self) -> &'static str {
    match self {
      Self::Home => "首页",
      Self::Explore => "发现",
      Self::Profile => "我的",
    }
  }

  fn description(self) -> &'static str {
    match self {
      Self::Home => "这里是你的首页，快速查看最近的动态。",
      Self::Explore => "浏览新内容，发现更多有趣的东西。",
      Self::Profile => "管理你的个人资料和应用设置。",
    }
  }
}

impl DrawerAction {
  fn title(self) -> &'static str {
    match self {
      Self::Overview => "已就绪",
      Self::Notifications => "通知设置",
      Self::Appearance => "外观偏好",
      Self::Security => "安全与隐私",
      Self::About => "关于应用",
      Self::Version => "版本信息",
      Self::Help => "使用帮助",
    }
  }

  fn description(self) -> &'static str {
    match self {
      Self::Overview => "从抽屉中快速切换导航、查看设置分组，或打开应用说明。",
      Self::Notifications => "管理提醒频率、消息推送和横幅通知的开启状态。",
      Self::Appearance => "调整主题风格、界面密度和阅读区域的显示偏好。",
      Self::Security => "查看登录设备、隐私权限和账号保护相关的入口。",
      Self::About => "查看应用简介、核心能力以及当前示例页面的用途。",
      Self::Version => "当前版本为 0.1.0，适合在这里展示构建号和更新日志入口。",
      Self::Help => "提供常见问题、操作说明和反馈渠道等帮助信息。",
    }
  }
}

struct App {
  drawer_open: bool,
  selected_tab: Tab,
  selected_action: DrawerAction,
}

impl App {
  fn open_drawer(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.drawer_open = true;
    cx.notify();
  }

  fn close_drawer(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.drawer_open = false;
    cx.notify();
  }

  fn drawer_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let selected_tab = self.selected_tab;
    let selected_action = self.selected_action;
    div()
      .absolute()
      .top(TITLE_BAR_HEIGHT)
      .left_0()
      .bottom_0()
      .w(px(320.))
      .bg(cx.theme().background)
      .border_r_1()
      .border_color(cx.theme().border)
      .shadow_xl()
      .child(
        v_flex()
          .size_full()
          .gap_4()
          .p_4()
          .child(
            h_flex()
              .w_full()
              .items_center()
              .justify_between()
              .child(div().font_semibold().child("导航"))
              .child(
                Button::new("close-drawer")
                  .ghost()
                  .small()
                  .label("关闭")
                  .on_click(cx.listener(Self::close_drawer)),
              ),
          )
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("从这里快速切换页面、查看设置入口和应用信息。"),
          )
          .child(
            v_flex()
              .gap_2()
              .child(div().text_sm().font_semibold().child("快捷导航"))
              .child(
                Button::new("drawer-home")
                  .ghost()
                  .small()
                  .w_full()
                  .label("首页")
                  .selected(selected_tab == Tab::Home)
                  .on_click(cx.listener(|this, event, window, cx| {
                    this.select_tab_from_drawer(Tab::Home, event, window, cx);
                  })),
              )
              .child(
                Button::new("drawer-explore")
                  .ghost()
                  .small()
                  .w_full()
                  .label("发现")
                  .selected(selected_tab == Tab::Explore)
                  .on_click(cx.listener(|this, event, window, cx| {
                    this.select_tab_from_drawer(Tab::Explore, event, window, cx);
                  })),
              )
              .child(
                Button::new("drawer-profile")
                  .ghost()
                  .small()
                  .w_full()
                  .label("我的")
                  .selected(selected_tab == Tab::Profile)
                  .on_click(cx.listener(|this, event, window, cx| {
                    this.select_tab_from_drawer(Tab::Profile, event, window, cx);
                  })),
              ),
          )
          .child(
            Accordion::new("drawer-sections")
              .multiple(true)
              .item(|item| {
                item
                  .title("设置")
                  .open(true)
                  .child(
                    v_flex()
                      .gap_2()
                      .child(
                        Button::new("drawer-notifications")
                          .ghost()
                          .small()
                          .w_full()
                          .label("通知设置")
                          .selected(selected_action == DrawerAction::Notifications)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::Notifications,
                              event,
                              window,
                              cx,
                            );
                          })),
                      )
                      .child(
                        Button::new("drawer-appearance")
                          .ghost()
                          .small()
                          .w_full()
                          .label("外观偏好")
                          .selected(selected_action == DrawerAction::Appearance)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::Appearance,
                              event,
                              window,
                              cx,
                            );
                          })),
                      )
                      .child(
                        Button::new("drawer-security")
                          .ghost()
                          .small()
                          .w_full()
                          .label("安全与隐私")
                          .selected(selected_action == DrawerAction::Security)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::Security,
                              event,
                              window,
                              cx,
                            );
                          })),
                      ),
                  )
              })
              .item(|item| {
                item
                  .title("查看信息")
                  .open(true)
                  .child(
                    v_flex()
                      .gap_2()
                      .child(
                        Button::new("drawer-about")
                          .ghost()
                          .small()
                          .w_full()
                          .label("关于应用")
                          .selected(selected_action == DrawerAction::About)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::About,
                              event,
                              window,
                              cx,
                            );
                          })),
                      )
                      .child(
                        Button::new("drawer-version")
                          .ghost()
                          .small()
                          .w_full()
                          .label("版本信息")
                          .selected(selected_action == DrawerAction::Version)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::Version,
                              event,
                              window,
                              cx,
                            );
                          })),
                      )
                      .child(
                        Button::new("drawer-help")
                          .ghost()
                          .small()
                          .w_full()
                          .label("使用帮助")
                          .selected(selected_action == DrawerAction::Help)
                          .on_click(cx.listener(|this, event, window, cx| {
                            this.select_drawer_action(
                              DrawerAction::Help,
                              event,
                              window,
                              cx,
                            );
                          })),
                      ),
                  )
              }),
          ),
      )
  }

  fn select_tab(&mut self, tab: Tab, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.selected_tab = tab;
    cx.notify();
  }

  fn select_tab_from_drawer(
    &mut self,
    tab: Tab,
    _: &ClickEvent,
    _: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.selected_tab = tab;
    self.drawer_open = false;
    cx.notify();
  }

  fn select_drawer_action(
    &mut self,
    action: DrawerAction,
    _: &ClickEvent,
    _: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.selected_action = action;
    self.drawer_open = false;
    cx.notify();
  }

  fn tab_button(&self, tab: Tab, id: &'static str, cx: &mut Context<Self>) -> Button {
    Button::new(id)
      .ghost()
      .small()
      .flex_1()
      .label(tab.title())
      .selected(self.selected_tab == tab)
      .on_click(cx.listener(move |this, event, window, cx| {
        this.select_tab(tab, event, window, cx);
      }))
  }
}

impl Render for App {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let tab = self.selected_tab;
    let action = self.selected_action;
    let drawer_open = self.drawer_open;

    v_flex()
      .size_full()
      .relative()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        TitleBar::new().child(
          h_flex().h_full().items_center().child("GPUI Kit"),
        ),
      )
      .child(
        v_flex()
          .flex_1()
          .p_8()
          .gap_4()
          .child(
            h_flex().w_full().items_center().child(
              Button::new("open-drawer")
                .ghost()
                .small()
                .label("☰")
                .on_click(cx.listener(Self::open_drawer)),
            ),
          )
          .child(div().text_2xl().font_semibold().child(tab.title()))
          .child(
            div()
              .text_color(cx.theme().muted_foreground)
              .child(tab.description()),
          )
          .child(
            div()
              .mt_4()
              .p_6()
              .rounded(cx.theme().radius)
              .border_1()
              .border_color(cx.theme().border)
              .bg(cx.theme().secondary)
              .child(format!("当前页面：{}", tab.title())),
          )
          .child(
            div()
              .p_6()
              .rounded(cx.theme().radius)
              .border_1()
              .border_color(cx.theme().border)
              .bg(cx.theme().secondary)
              .child(
                v_flex()
                  .gap_2()
                  .child(div().font_semibold().child(action.title()))
                  .child(
                    div()
                      .text_color(cx.theme().muted_foreground)
                      .child(action.description()),
                  ),
              ),
          ),
      )
      .child(
        h_flex()
          .w_full()
          .h(px(72.))
          .px_4()
          .gap_2()
          .border_t_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().secondary)
          .child(self.tab_button(Tab::Home, "home-tab", cx))
          .child(self.tab_button(Tab::Explore, "explore-tab", cx))
          .child(self.tab_button(Tab::Profile, "profile-tab", cx)),
      )
          .when(drawer_open, |this| this.child(self.drawer_panel(cx)))
  }
}

fn main() {
  let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

  app.run(move |cx| {
    gpui_kit::init(cx);

    cx.spawn(async move |cx| {
      cx.open_window(TitleBar::window_options(), |window, cx| {
        let view = cx.new(|_| App {
          drawer_open: false,
          selected_tab: Tab::Home,
          selected_action: DrawerAction::Overview,
        });
        cx.new(|cx| Root::new(view, window, cx))
      })
      .expect("failed to open window");
    })
    .detach();
  });
}
