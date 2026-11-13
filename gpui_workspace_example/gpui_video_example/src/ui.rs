use gpui_kit::assets::IconName as UiIcon;

use crate::*;

const ACCENT: u32 = 0x58b4ff;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
  Playing,
  Library,
  Settings,
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
  background: Hsla,
  pub panel: Hsla,
  elevated: Hsla,
  border: Hsla,
  text: Hsla,
  muted: Hsla,
}

pub(super) fn apply_theme(light: bool, window: Option<&mut Window>, cx: &mut App) {
  Theme::change(
    if light {
      ThemeMode::Light
    } else {
      ThemeMode::Dark
    },
    window,
    cx,
  );
  Theme::update(cx, |theme| {
    theme.colors.primary = rgb(ACCENT).into();
    theme.colors.primary_hover = rgb(0x80c6ff).into();
    theme.colors.primary_active = rgb(0x3298ed).into();
    theme.colors.primary_foreground = rgb(0x121212).into();
    theme.colors.button_primary = rgb(ACCENT).into();
    theme.colors.button_primary_hover = rgb(0x80c6ff).into();
    theme.colors.button_primary_active = rgb(0x3298ed).into();
    theme.colors.button_primary_foreground = rgb(0x121212).into();
    theme.colors.slider_bar = rgb(ACCENT).into();
    theme.colors.slider_thumb = rgb(ACCENT).into();
  });
}

fn icon(name: UiIcon) -> Icon {
  Icon::new(name).size(px(18.0))
}

fn media_title(path: &Path) -> String {
  path
    .file_stem()
    .unwrap_or(path.as_os_str())
    .to_string_lossy()
    .into_owned()
}

fn landscape() -> Div {
  div()
    .relative()
    .size_full()
    .overflow_hidden()
    .bg(rgb(0x18252e))
    .child(
      div()
        .absolute()
        .left(relative(0.65))
        .top(relative(0.20))
        .w(relative(0.09))
        .aspect_ratio(1.0)
        .rounded_full()
        .bg(rgb(0xffb376)),
    )
    .child(
      canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
          for (color, points) in [
            (
              0x334957,
              vec![
                (0.0, 0.56),
                (0.16, 0.27),
                (0.28, 0.43),
                (0.43, 0.23),
                (0.60, 0.51),
                (0.81, 0.36),
                (1.0, 0.48),
                (1.0, 1.0),
                (0.0, 1.0),
              ],
            ),
            (
              0x243c49,
              vec![
                (0.0, 0.64),
                (0.20, 0.48),
                (0.33, 0.56),
                (0.51, 0.40),
                (0.69, 0.60),
                (0.86, 0.48),
                (1.0, 0.59),
                (1.0, 1.0),
                (0.0, 1.0),
              ],
            ),
            (
              0x163d55,
              vec![(0.0, 0.69), (1.0, 0.69), (1.0, 1.0), (0.0, 1.0)],
            ),
            (
              0x102c3b,
              vec![
                (0.0, 0.58),
                (0.18, 0.74),
                (0.34, 0.77),
                (0.22, 0.83),
                (0.0, 0.91),
              ],
            ),
            (
              0x0e2633,
              vec![
                (1.0, 0.62),
                (0.84, 0.76),
                (0.75, 0.82),
                (0.88, 0.88),
                (1.0, 0.89),
              ],
            ),
            (
              0x1b465c,
              vec![
                (0.0, 0.94),
                (0.27, 0.89),
                (0.49, 0.97),
                (0.75, 0.92),
                (1.0, 0.97),
                (1.0, 1.0),
                (0.0, 1.0),
              ],
            ),
          ] {
            let mut path = PathBuilder::fill();
            for (index, (horizontal, vertical)) in points.into_iter().enumerate() {
              let position = point(
                bounds.origin.x + bounds.size.width * horizontal,
                bounds.origin.y + bounds.size.height * vertical,
              );
              if index == 0 {
                path.move_to(position);
              } else {
                path.line_to(position);
              }
            }
            path.close();
            if let Ok(path) = path.build() {
              window.paint_path(path, rgb(color));
            }
          }
        },
      )
      .absolute()
      .size_full(),
    )
}

impl VideoPlayer {
  pub(super) fn palette(&self) -> Palette {
    if self.settings.light_theme {
      Palette {
        background: rgb(0xf3f5f7).into(),
        panel: rgb(0xffffff).into(),
        elevated: rgb(0xe9edf1).into(),
        border: rgb(0xdce1e7).into(),
        text: rgb(0x202833).into(),
        muted: rgb(0x637080).into(),
      }
    } else {
      Palette {
        background: rgb(0x121212).into(),
        panel: rgb(0x1e1e1e).into(),
        elevated: rgb(0x2a2a2a).into(),
        border: rgb(0x303030).into(),
        text: rgb(0xe0e0e0).into(),
        muted: rgb(0x929292).into(),
      }
    }
  }

  fn action_button(&self, id: &'static str, name: UiIcon) -> Button {
    let palette = self.palette();
    Button::new(id)
      .icon(icon(name))
      .ghost()
      .bg(palette.elevated)
  }

  fn open_button(&self, id: &'static str, cx: &mut Context<Self>) -> Button {
    Button::new(id)
      .primary()
      .icon(icon(UiIcon::FolderOpen))
      .label("打开视频")
      .disabled(self.choosing_file)
      .on_click(cx.listener(|this, _, window, cx| this.open_file(OpenFileKind::Video, window, cx)))
  }

  fn sidebar(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    div()
      .w(px(if compact { 76.0 } else { 228.0 }))
      .flex_shrink_0()
      .h_full()
      .flex()
      .flex_col()
      .px_4()
      .py_6()
      .border_r_1()
      .border_color(palette.border)
      .child(
        div()
          .flex()
          .items_center()
          .gap_3()
          .px_2()
          .mb_8()
          .child(
            div()
              .size(px(32.0))
              .rounded_lg()
              .bg(rgb(ACCENT))
              .text_color(rgb(0x121212))
              .flex()
              .items_center()
              .justify_center()
              .child(icon(UiIcon::Film)),
          )
          .when(!compact, |element| {
            element.child(
              div()
                .child(div().text_lg().font_weight(FontWeight::BOLD).child("映刻"))
                .child(
                  div()
                    .text_size(px(10.0))
                    .text_color(palette.muted)
                    .child("F R A M E"),
                ),
            )
          }),
      )
      .child(
        div().flex().flex_col().gap_3().children(
          [
            (Page::Playing, UiIcon::CirclePlay, "正在播放"),
            (Page::Library, UiIcon::Library, "媒体库"),
            (Page::Settings, UiIcon::Settings, "播放设置"),
          ]
          .into_iter()
          .enumerate()
          .map(|(index, (page, name, label))| {
            let selected = self.page == page;
            div()
              .id(("navigation", index))
              .flex()
              .items_center()
              .gap_3()
              .h(px(52.0))
              .px_3()
              .rounded_md()
              .cursor_pointer()
              .bg(if selected {
                palette.elevated
              } else {
                palette.background
              })
              .text_color(if selected {
                rgb(ACCENT).into()
              } else {
                palette.muted
              })
              .hover(move |style| style.bg(palette.elevated).text_color(palette.text))
              .child(icon(name))
              .when(!compact, |element| {
                element.child(div().font_weight(FontWeight::MEDIUM).child(label))
              })
              .on_click(cx.listener(move |this, _, window, cx| {
                this.save();
                this.page = page;
                window.focus(&this.focus, cx);
                cx.notify();
              }))
          }),
        ),
      )
      .child(div().flex_1())
      .when(!compact, |element| {
        element.child(
          div()
            .p_4()
            .rounded_lg()
            .bg(palette.panel)
            .child(div().text_sm().child("好故事，值得慢慢看。"))
            .child(
              div()
                .mt_2()
                .text_xs()
                .text_color(palette.muted)
                .child("拖入本地视频，留住每一帧。"),
            )
            .child(div().mt_4().child(self.open_button("sidebar-open", cx))),
        )
      })
      .child(
        div()
          .mt_6()
          .px_2()
          .text_xs()
          .text_color(palette.muted)
          .child(if compact {
            "本地"
          } else {
            "本地播放 · 专注每一帧"
          }),
      )
  }

  fn speed_button(&self, id: &'static str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let speed = self.settings.speed;
    let player = cx.weak_entity();
    Button::new(id)
      .ghost()
      .label(format!("{speed}×"))
      .tooltip("播放速度 · 音频保持音调")
      .disabled(self.loading || self.choosing_file)
      .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
        for &value in PLAYBACK_SPEEDS {
          let player = player.clone();
          menu = menu.item(
            PopupMenuItem::new(format!("{value}×"))
              .checked(value == speed)
              .on_click(move |_, window, cx| {
                let _ = player.update(cx, |this, cx| {
                  this.set_speed(value, window);
                  cx.notify();
                });
              }),
          );
        }
        menu
      })
  }

  fn subtitle_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let tracks = self
      .info
      .as_ref()
      .map(|info| info.subtitle_tracks.clone())
      .unwrap_or_default();
    let enabled = self.settings.subtitles.enabled;
    let player = cx.weak_entity();
    let loaded = self.subtitle_track.is_some();
    Button::new("transport-subtitles")
      .ghost()
      .icon(icon(UiIcon::Captions))
      .label(if loaded && enabled {
        "字幕已开"
      } else {
        "字幕"
      })
      .disabled(self.state.is_none() || self.loading || self.choosing_file)
      .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
        let toggle_player = player.clone();
        menu = menu.item(PopupMenuItem::new("显示字幕").checked(enabled).on_click(
          move |_, _, cx| {
            let _ = toggle_player.update(cx, |this, cx| {
              this.settings.subtitles.enabled = !this.settings.subtitles.enabled;
              this.save();
              cx.notify();
            });
          },
        ));
        let open_player = player.clone();
        menu = menu.item(
          PopupMenuItem::new("加载外挂 SRT…").on_click(move |_, window, cx| {
            let _ = open_player.update(cx, |this, cx| {
              this.open_file(OpenFileKind::Subtitles, window, cx)
            });
          }),
        );
        for track in &tracks {
          let player = player.clone();
          let index = track.index;
          menu = menu.item(
            PopupMenuItem::new(track.label.clone())
              .disabled(!track.text)
              .on_click(move |_, _, cx| {
                let _ = player.update(cx, |this, cx| {
                  this.engine.send(Operation::Embedded(index));
                  this.notice = Some("正在提取内嵌字幕…".into());
                  cx.notify();
                });
              }),
          );
        }
        menu
      })
  }

  fn transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    let busy = self.loading || self.choosing_file;
    let paused = self
      .state
      .as_ref()
      .is_none_or(|state| state.paused || state.ended);
    div()
      .rounded_lg()
      .bg(palette.panel)
      .border_1()
      .border_color(palette.border)
      .child(self.timeline(cx))
      .child(
        div()
          .flex()
          .flex_wrap()
          .items_center()
          .gap_2()
          .px_3()
          .pb_3()
          .child(
            self
              .action_button("transport-previous", UiIcon::SkipBack)
              .rounded(px(20.0))
              .tooltip("上一个视频")
              .disabled(self.playlist.len() < 2 || busy)
              .on_click(cx.listener(|this, _, window, cx| {
                this.navigate(-1, window);
                cx.notify();
              })),
          )
          .child(
            Button::new("transport-play")
              .primary()
              .rounded(px(24.0))
              .size(px(44.0))
              .icon(icon(if paused { UiIcon::Play } else { UiIcon::Pause }))
              .tooltip(if paused {
                "播放 / 重播 · 空格"
              } else {
                "暂停 · 空格"
              })
              .disabled(self.state.is_none() || busy)
              .on_click(cx.listener(|this, _, window, cx| this.toggle_paused(window, cx))),
          )
          .child(
            self
              .action_button("transport-next", UiIcon::SkipForward)
              .rounded(px(20.0))
              .tooltip("下一个视频")
              .disabled(self.playlist.len() < 2 || busy)
              .on_click(cx.listener(|this, _, window, cx| {
                this.navigate(1, window);
                cx.notify();
              })),
          )
          .child(div().w(px(1.0)).h(px(20.0)).mx_1().bg(palette.border))
          .child(
            self
              .action_button(
                "transport-mute",
                if self.settings.muted {
                  UiIcon::VolumeX
                } else {
                  UiIcon::Volume2
                },
              )
              .tooltip("静音 / 取消静音 · M")
              .on_click(cx.listener(|this, _, _, cx| {
                this.settings.muted = !this.settings.muted;
                this.update_volume();
                this.save();
                cx.notify();
              })),
          )
          .child(
            div()
              .w(px(78.0))
              .child(Slider::new(&self.volume_slider).accessibility_label("音量")),
          )
          .child(
            div()
              .w(px(34.0))
              .text_xs()
              .text_color(palette.muted)
              .child(format!(
                "{:.0}%",
                if self.settings.muted {
                  0.0
                } else {
                  self.settings.volume * 100.0
                }
              )),
          )
          .child(div().flex_1())
          .child(self.speed_button("transport-speed", cx))
          .child(self.subtitle_button(cx))
          .child(
            self
              .action_button("transport-pip", UiIcon::PictureInPicture)
              .disabled(true)
              .tooltip("画中画暂不支持"),
          )
          .child(
            self
              .action_button("transport-fullscreen", UiIcon::Expand)
              .tooltip("全屏 · F / Esc")
              .on_click(cx.listener(|_, _, window, _| window.toggle_fullscreen())),
          ),
      )
  }

  fn stage(
    &self,
    subtitle: Option<String>,
    fullscreen: bool,
    height: f32,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    div()
      .id("video-surface")
      .relative()
      .w_full()
      .overflow_hidden()
      .bg(rgb(0x000000))
      .when(fullscreen, |element| element.flex_1().min_h_0())
      .when(!fullscreen, |element| element.h(px(height)).rounded_lg())
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, event: &MouseDownEvent, window, cx| {
          window.focus(&this.focus, cx);
          if event.click_count == 2 {
            window.toggle_fullscreen();
          }
        }),
      )
      .when(self.video_view.size().is_some(), |element| {
        element.child(self.video_view.element())
      })
      .when(self.video_view.size().is_none(), |element| {
        element
          .child(landscape())
          .child(
            div()
              .absolute()
              .top_6()
              .left_6()
              .text_color(rgb(0xffffff))
              .child(div().text_xs().child("映刻精选视觉 · DESIGN PREVIEW"))
              .child(
                div()
                  .mt_2()
                  .text_size(px(30.0))
                  .font_weight(FontWeight::SEMIBOLD)
                  .child("山海之间"),
              )
              .child(
                div()
                  .mt_2()
                  .text_sm()
                  .text_color(rgb(0xbac8d0))
                  .child("让每一帧，都值得停留。"),
              ),
          )
          .when(!self.loading, |element| {
            element.child(
              div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                  Button::new("cover-open")
                    .icon(Icon::new(UiIcon::Play).size(px(25.0)))
                    .size(px(64.0))
                    .rounded(px(32.0))
                    .tooltip("打开本地视频")
                    .disabled(self.choosing_file)
                    .on_click(cx.listener(|this, _, window, cx| {
                      this.open_file(OpenFileKind::Video, window, cx)
                    })),
                ),
            )
          })
          .child(
            div()
              .absolute()
              .bottom_5()
              .left_6()
              .text_xs()
              .text_color(rgb(0xc6d5de))
              .child(if self.loading {
                "正在准备画面…"
              } else {
                "设计封面 · 未加载媒体 · 拖入视频或点击播放按钮打开"
              }),
          )
      })
      .when_some(subtitle, |element, text| {
        element.child(self.subtitle_overlay(text))
      })
      .when(self.loading, |element| {
        element.child(
          div()
            .absolute()
            .top_4()
            .right_4()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgba(0x000000bb))
            .text_sm()
            .text_color(rgb(0xffffff))
            .child("正在加载 / 跳转…"),
        )
      })
  }

  fn toggle_favorite(&mut self, path: PathBuf) {
    if self.settings.favorites.contains(&path) {
      self.settings.favorites.retain(|entry| entry != &path);
    } else {
      self.settings.favorites.push(path);
    }
    self.save();
  }

  fn media_details(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    let current_chapter = self
      .info
      .as_ref()
      .zip(self.state.as_ref())
      .and_then(|(info, state)| {
        info
          .chapters
          .iter()
          .rposition(|chapter| chapter.start <= self.scrub_target.unwrap_or(state.time))
      });
    let title = self
      .path
      .as_deref()
      .map(media_title)
      .unwrap_or_else(|| "你的下一场光影之旅".into());
    let metadata = self
      .info
      .as_ref()
      .map(|info| {
        let (width, height) = info.display_size();
        format!(
          "{width} × {height}  ·  {:.2} FPS  ·  {}",
          info.fps,
          format_time(info.duration)
        )
      })
      .unwrap_or_else(|| "打开一个本地视频，或将多个文件拖入窗口创建播放列表。".into());
    div()
      .mt_5()
      .child(
        div()
          .flex()
          .items_center()
          .gap_3()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .text_lg()
              .font_weight(FontWeight::SEMIBOLD)
              .truncate()
              .child(title),
          )
          .child(
            self
              .action_button("favorite-current", UiIcon::Heart)
              .label(
                if self
                  .path
                  .as_ref()
                  .is_some_and(|path| self.settings.favorites.contains(path))
                {
                  "已收藏"
                } else {
                  "收藏"
                },
              )
              .disabled(self.path.is_none())
              .on_click(cx.listener(|this, _, _, cx| {
                if let Some(path) = this.path.clone() {
                  this.toggle_favorite(path);
                  cx.notify();
                }
              })),
          ),
      )
      .child(
        div()
          .mt_2()
          .text_sm()
          .text_color(palette.muted)
          .child(metadata),
      )
      .when_some(self.state.as_ref(), |element, state| {
        element.child(
          div()
            .mt_3()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(palette.muted)
            .child(div().size(px(6.0)).rounded_full().bg(rgb(ACCENT)))
            .child(format!(
              "{} · {} · 第 {} 帧{}",
              if state.ended {
                "播放结束"
              } else if state.paused {
                "已暂停"
              } else {
                "正在播放"
              },
              state.backend,
              state.frame_number,
              if state.audible { "" } else { " · 无声" }
            )),
        )
      })
      .child(
        div()
          .mt_5()
          .pt_4()
          .border_t_1()
          .border_color(palette.border)
          .child(
            div()
              .flex()
              .items_center()
              .gap_2()
              .text_sm()
              .font_weight(FontWeight::SEMIBOLD)
              .child(icon(UiIcon::ListVideo))
              .child("章节清单"),
          )
          .children(
            self
              .info
              .as_ref()
              .into_iter()
              .flat_map(|info| info.chapters.iter())
              .enumerate()
              .map(|(index, chapter)| {
                let start = chapter.start;
                div()
                  .id(("chapter", index))
                  .mt_2()
                  .p_3()
                  .rounded_md()
                  .cursor_pointer()
                  .bg(if current_chapter == Some(index) {
                    palette.elevated
                  } else {
                    palette.panel
                  })
                  .hover(move |style| style.bg(palette.elevated))
                  .flex()
                  .items_center()
                  .gap_3()
                  .child(
                    div()
                      .text_xs()
                      .text_color(rgb(ACCENT))
                      .child(format_time(start)),
                  )
                  .child(
                    div()
                      .flex_1()
                      .min_w_0()
                      .truncate()
                      .text_sm()
                      .child(chapter.title.clone()),
                  )
                  .child(icon(UiIcon::ChevronRight))
                  .on_click(cx.listener(move |this, _, window, cx| {
                    if this.state.is_some() && !this.loading && !this.choosing_file {
                      this.seek_to(start, window);
                      cx.notify();
                    }
                  }))
              }),
          )
          .when(
            self
              .info
              .as_ref()
              .is_none_or(|info| info.chapters.is_empty()),
            |element| {
              element.child(div().mt_3().text_sm().text_color(palette.muted).child(
                if self.info.is_some() {
                  "此视频没有内嵌章节，可以使用进度条定位。"
                } else {
                  "加载媒体后，在这里查看视频的内嵌章节。"
                },
              ))
            },
          ),
      )
  }

  fn playlist_panel(&self, narrow: bool, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    let current = self
      .path
      .as_ref()
      .and_then(|path| self.playlist.iter().position(|entry| entry == path));
    div()
      .flex_shrink_0()
      .when(!narrow, |element| element.w(px(272.0)))
      .when(narrow, |element| element.w_full().mt_6())
      .child(
        div()
          .flex()
          .items_center()
          .justify_between()
          .child(div().font_weight(FontWeight::SEMIBOLD).child("接下来播放"))
          .child(
            div()
              .text_xs()
              .text_color(palette.muted)
              .child(format!("{} 个视频", self.playlist.len())),
          ),
      )
      .child(
        div()
          .mt_4()
          .flex()
          .flex_col()
          .gap_2()
          .children(self.playlist.iter().enumerate().map(|(index, path)| {
            let selected = current == Some(index);
            let path = path.clone();
            div()
              .id(("queue", index))
              .p_3()
              .rounded_md()
              .cursor_pointer()
              .bg(if selected {
                rgb(ACCENT).into()
              } else {
                palette.panel
              })
              .text_color(if selected {
                rgb(0x121212).into()
              } else {
                palette.text
              })
              .hover(move |style| {
                style.bg(if selected {
                  rgb(0x80c6ff).into()
                } else {
                  palette.elevated
                })
              })
              .flex()
              .items_center()
              .gap_3()
              .child(div().w(px(22.0)).text_sm().child(if selected {
                "▶".into()
              } else {
                format!("{:02}", index + 1)
              }))
              .child(
                div()
                  .flex_1()
                  .min_w_0()
                  .child(div().text_sm().truncate().child(media_title(&path)))
                  .child(div().mt_1().text_xs().opacity(0.65).child(if selected {
                    "正在播放"
                  } else {
                    "本地视频"
                  })),
              )
              .on_click(cx.listener(move |this, _, window, cx| {
                if !this.choosing_file {
                  this.open_path(&path, None, window);
                  cx.notify();
                }
              }))
          })),
      )
      .when(self.playlist.is_empty(), |element| {
        element.child(
          div()
            .mt_4()
            .p_5()
            .rounded_lg()
            .border_1()
            .border_color(palette.border)
            .child(
              div()
                .mb_3()
                .text_color(palette.muted)
                .child(icon(UiIcon::ListVideo)),
            )
            .child(div().text_sm().child("还没有待播视频"))
            .child(
              div()
                .mt_2()
                .text_xs()
                .text_color(palette.muted)
                .child("一次打开多个文件，按你的顺序连续播放。"),
            )
            .child(div().mt_4().child(self.open_button("queue-open", cx))),
        )
      })
      .child(
        div()
          .mt_5()
          .p_4()
          .rounded_lg()
          .bg(palette.panel)
          .child(
            Checkbox::new("queue-repeat")
              .label("循环当前视频")
              .checked(self.settings.repeat)
              .on_change(cx.listener(|this, value: &bool, _, cx| {
                this.settings.repeat = *value;
                this.engine.send(Operation::Repeat(*value));
                this.save();
                cx.notify();
              })),
          )
          .child(
            div()
              .mt_3()
              .text_xs()
              .text_color(palette.muted)
              .child("关闭循环后，播放结束将自动进入下一项。"),
          ),
      )
      .child(
        div()
          .mt_6()
          .pt_4()
          .border_t_1()
          .border_color(palette.border)
          .text_xs()
          .text_color(palette.muted)
          .child(
            div()
              .flex()
              .items_center()
              .gap_2()
              .child(icon(UiIcon::Keyboard))
              .child("快捷键"),
          )
          .child(div().mt_3().child("空格  播放 / 暂停"))
          .child(div().mt_2().child("← / →  跳转 5 秒"))
          .child(div().mt_2().child("F  全屏    M  静音")),
      )
  }

  fn playing_page(
    &self,
    subtitle: Option<String>,
    viewport: gpui_kit::Size<Pixels>,
    compact: bool,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let narrow = viewport.width < px(1280.0);
    let sidebar_width = if compact { 76.0 } else { 228.0 };
    let content_width =
      f32::from(viewport.width) - sidebar_width - 56.0 - if narrow { 0.0 } else { 296.0 };
    let height = (content_width * 9.0 / 16.0).clamp(230.0, 585.0);
    div()
      .flex()
      .gap_6()
      .when(narrow, |element| element.flex_col())
      .child(
        div()
          .flex_1()
          .min_w_0()
          .child(self.stage(subtitle, false, height, cx))
          .child(div().mt_3().child(self.transport(cx)))
          .child(self.media_details(cx)),
      )
      .child(self.playlist_panel(narrow, cx))
  }

  fn library_card(
    &self,
    path: PathBuf,
    index: usize,
    group: &'static str,
    cx: &mut Context<Self>,
  ) -> impl IntoElement + use<> {
    let palette = self.palette();
    let recent = self.settings.recent.iter().find(|entry| entry.path == path);
    let position = recent.map_or(0.0, |entry| entry.position);
    let duration = recent.map_or(0.0, |entry| entry.duration);
    let progress = if duration > 0.0 {
      (position / duration).clamp(0.0, 1.0) as f32
    } else {
      0.0
    };
    let favorite = self.settings.favorites.contains(&path);
    let favorite_path = path.clone();
    let available = path.is_file();
    div()
      .id((group, index))
      .flex()
      .items_center()
      .gap_4()
      .p_4()
      .rounded_lg()
      .bg(palette.panel)
      .child(
        div()
          .w(px(96.0))
          .h(px(64.0))
          .flex_shrink_0()
          .rounded_md()
          .overflow_hidden()
          .child(landscape()),
      )
      .child(
        div()
          .flex_1()
          .min_w_0()
          .child(
            div()
              .text_sm()
              .font_weight(FontWeight::MEDIUM)
              .truncate()
              .child(media_title(&path)),
          )
          .child(
            div()
              .mt_1()
              .text_xs()
              .text_color(palette.muted)
              .truncate()
              .child(path.display().to_string()),
          )
          .child(
            div()
              .mt_2()
              .flex()
              .items_center()
              .gap_3()
              .child(
                div()
                  .text_xs()
                  .text_color(palette.muted)
                  .child(if duration > 0.0 {
                    format!(
                      "{} / {} · 已观看 {:.0}%",
                      format_time(Duration::try_from_secs_f64(position).unwrap_or_default()),
                      format_time(Duration::try_from_secs_f64(duration).unwrap_or_default()),
                      progress * 100.0
                    )
                  } else {
                    format!(
                      "上次播放至 {}",
                      format_time(Duration::try_from_secs_f64(position).unwrap_or_default())
                    )
                  }),
              )
              .when(!available, |element| {
                element.child(
                  div()
                    .text_xs()
                    .text_color(rgb(0xdf9d62))
                    .child("文件已移动或不存在"),
                )
              }),
          )
          .child(
            div()
              .mt_2()
              .h(px(3.0))
              .w_full()
              .rounded_full()
              .bg(palette.elevated)
              .child(
                div()
                  .h_full()
                  .w(relative(progress))
                  .rounded_full()
                  .bg(rgb(ACCENT)),
              ),
          ),
      )
      .child(
        Button::new((
          if group == "recent" {
            "recent-favorite"
          } else {
            "saved-favorite"
          },
          index,
        ))
        .ghost()
        .icon(icon(UiIcon::Heart))
        .tooltip(if favorite { "取消收藏" } else { "收藏" })
        .when(favorite, |button| button.text_color(rgb(ACCENT)))
        .on_click(cx.listener(move |this, _, _, cx| {
          this.toggle_favorite(favorite_path.clone());
          cx.notify();
        })),
      )
      .child(
        Button::new((
          if group == "recent" {
            "recent-resume"
          } else {
            "saved-resume"
          },
          index,
        ))
        .ghost()
        .icon(icon(UiIcon::Play))
        .tooltip("继续播放")
        .disabled(!available || self.choosing_file)
        .on_click(cx.listener(move |this, _, window, cx| {
          if !this.playlist.contains(&path) {
            this.playlist.push(path.clone());
          }
          this.open_path(&path, None, window);
          cx.notify();
        })),
      )
  }

  fn library_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    div()
      .flex()
      .flex_col()
      .gap_4()
      .child(
        div()
          .text_sm()
          .text_color(palette.muted)
          .child("从上次停留的那一帧继续。列表封面为统一示意图。"),
      )
      .child(
        div()
          .mt_3()
          .text_lg()
          .font_weight(FontWeight::SEMIBOLD)
          .child("最近观看"),
      )
      .children(
        self
          .settings
          .recent
          .iter()
          .enumerate()
          .map(|(index, entry)| self.library_card(entry.path.clone(), index, "recent", cx)),
      )
      .when(self.settings.recent.is_empty(), |element| {
        element.child(self.empty_card(
          "还没有观看记录",
          "打开本地视频后，播放进度会自动保存在这里。",
        ))
      })
      .child(
        div()
          .mt_6()
          .text_lg()
          .font_weight(FontWeight::SEMIBOLD)
          .child("收藏夹"),
      )
      .children(
        self
          .settings
          .favorites
          .iter()
          .enumerate()
          .map(|(index, path)| self.library_card(path.clone(), index, "favorites", cx)),
      )
      .when(self.settings.favorites.is_empty(), |element| {
        element.child(self.empty_card("收藏喜欢的光影", "点击视频旁的爱心，将它加入你的专属片单。"))
      })
  }

  fn empty_card(&self, title: &'static str, description: &'static str) -> Div {
    let palette = self.palette();
    div()
      .p_6()
      .rounded_lg()
      .bg(palette.panel)
      .child(div().text_sm().child(title))
      .child(
        div()
          .mt_2()
          .text_sm()
          .text_color(palette.muted)
          .child(description),
      )
  }

  fn settings_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let palette = self.palette();
    let quality = self
      .info
      .as_ref()
      .map(|info| {
        let (width, height) = info.display_size();
        format!("{width} × {height} · 原始画质")
      })
      .unwrap_or_else(|| "未加载媒体".into());
    div()
      .flex()
      .flex_col()
      .gap_5()
      .child(
        div()
          .text_sm()
          .text_color(palette.muted)
          .child("调整你的观影偏好，设置会自动保存。"),
      )
      .child(
        div()
          .p_5()
          .rounded_lg()
          .bg(palette.panel)
          .child(
            div()
              .mb_4()
              .font_weight(FontWeight::SEMIBOLD)
              .child("播放与声音"),
          )
          .child(
            div()
              .flex()
              .flex_wrap()
              .items_center()
              .gap_4()
              .child(div().w(px(90.0)).text_sm().child("播放速度"))
              .child(self.speed_button("settings-speed", cx)),
          )
          .child(
            div()
              .mt_4()
              .flex()
              .flex_wrap()
              .items_center()
              .gap_4()
              .child(div().w(px(90.0)).text_sm().child("视频画质"))
              .child(div().text_sm().child(quality)),
          )
          .child(
            div()
              .mt_2()
              .text_xs()
              .text_color(palette.muted)
              .child("本地文件按原始分辨率播放，不提供伪画质切换或 HDR 色调映射。"),
          )
          .child(
            div()
              .mt_5()
              .flex()
              .items_center()
              .gap_4()
              .child(div().w(px(90.0)).text_sm().child("音量"))
              .child(
                div()
                  .w(px(180.0))
                  .child(Slider::new(&self.volume_slider).accessibility_label("音量")),
              )
              .child(
                div()
                  .text_sm()
                  .child(format!("{:.0}%", self.settings.volume * 100.0)),
              ),
          ),
      )
      .child(
        div()
          .p_5()
          .rounded_lg()
          .bg(palette.panel)
          .child(
            div()
              .mb_4()
              .font_weight(FontWeight::SEMIBOLD)
              .child("字幕样式"),
          )
          .child(
            Checkbox::new("settings-subtitles-enabled")
              .label("显示字幕")
              .checked(self.settings.subtitles.enabled)
              .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                this.settings.subtitles.enabled = *enabled;
                this.save();
                cx.notify();
              })),
          )
          .child(div().mt_4().child(self.subtitle_settings(cx))),
      )
      .child(
        div()
          .p_5()
          .rounded_lg()
          .bg(palette.panel)
          .child(
            div()
              .mb_4()
              .font_weight(FontWeight::SEMIBOLD)
              .child("外观主题"),
          )
          .child(
            div().flex().gap_3().children(
              [(false, UiIcon::Moon, "深色"), (true, UiIcon::Sun, "浅色")]
                .into_iter()
                .enumerate()
                .map(|(index, (light, name, label))| {
                  Button::new(("theme", index))
                    .icon(icon(name))
                    .label(label)
                    .selected(self.settings.light_theme == light)
                    .on_click(cx.listener(move |this, _, window, cx| {
                      this.settings.light_theme = light;
                      apply_theme(light, Some(window), cx);
                      this.save();
                      cx.notify();
                    }))
                }),
            ),
          ),
      )
      .child(
        div()
          .p_5()
          .rounded_lg()
          .bg(palette.panel)
          .child(
            div()
              .mb_2()
              .font_weight(FontWeight::SEMIBOLD)
              .child("更多播放选项"),
          )
          .child(
            div()
              .mb_3()
              .text_xs()
              .text_color(palette.muted)
              .child("音轨、内嵌字幕、逐帧播放和截图等原有功能均保留。"),
          )
          .child(self.advanced_controls(cx)),
      )
  }

  pub(super) fn render_shell(
    &self,
    subtitle: Option<String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let palette = self.palette();
    let viewport = window.viewport_size();
    let compact = viewport.width < px(1050.0);
    let fullscreen = window.is_fullscreen();
    let title = match self.page {
      Page::Playing => "正在播放",
      Page::Library => "媒体库",
      Page::Settings => "播放设置",
    };
    div()
      .id("video-player")
      .size_full()
      .flex()
      .bg(palette.background)
      .text_color(palette.text)
      .text_size(px(14.0))
      .track_focus(&self.focus)
      .on_key_down(cx.listener(Self::key_down))
      .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
        this.open_files(paths.paths().to_vec(), window);
        cx.notify();
      }))
      .when(fullscreen, |element| {
        element.child(
          div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x000000))
            .child(self.stage(subtitle.clone(), true, 0.0, cx))
            .when_some(
              self.error.clone().or_else(|| self.notice.clone()),
              |element, message| {
                element.child(
                  div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(palette.panel)
                    .child(div().flex_1().min_w_0().text_sm().child(message))
                    .child(
                      Button::new("fullscreen-dismiss")
                        .ghost()
                        .icon(icon(UiIcon::X))
                        .tooltip("关闭提示")
                        .on_click(cx.listener(|this, _, _, cx| {
                          this.error = None;
                          this.notice = None;
                          cx.notify();
                        })),
                    ),
                )
              },
            )
            .child(self.transport(cx)),
        )
      })
      .when(!fullscreen, |element| {
        element.child(self.sidebar(compact, cx)).child(
          div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
              div()
                .h(px(80.0))
                .flex_shrink_0()
                .px_6()
                .flex()
                .items_center()
                .gap_3()
                .child(
                  div()
                    .flex_1()
                    .min_w_0()
                    .child(
                      div()
                        .text_xs()
                        .text_color(palette.muted)
                        .child("映刻 / FRAME"),
                    )
                    .child(
                      div()
                        .mt_1()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                    ),
                )
                .child(self.open_button("header-open", cx)),
            )
            .when_some(
              self.error.clone().or_else(|| self.notice.clone()),
              |element, message| {
                element.child(
                  div()
                    .mx_6()
                    .mb_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(if self.error.is_some() {
                      rgb(0x48242b)
                    } else {
                      rgb(0x263344)
                    })
                    .text_color(rgb(0xe0e0e0))
                    .child(
                      div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .whitespace_normal()
                        .child(message),
                    )
                    .child(
                      Button::new("dismiss")
                        .ghost()
                        .icon(icon(UiIcon::X))
                        .tooltip("关闭提示")
                        .on_click(cx.listener(|this, _, _, cx| {
                          this.error = None;
                          this.notice = None;
                          cx.notify();
                        })),
                    ),
                )
              },
            )
            .child(
              div()
                .id("page-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_6()
                .pb_6()
                .child(match self.page {
                  Page::Playing => self
                    .playing_page(subtitle, viewport, compact, cx)
                    .into_any_element(),
                  Page::Library => self.library_page(cx).into_any_element(),
                  Page::Settings => self.settings_page(cx).into_any_element(),
                }),
            ),
        )
      })
  }
}
