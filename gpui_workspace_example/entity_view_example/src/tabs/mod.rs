use gpui_kit::{
  AnyView, App, AppContext as _, Entity, Hsla, Pixels, Render, Window,
  component::dock::PanelControl, px,
};

mod accordion_tab;
mod alert_dialog_tab;
mod alert_tab;
mod attachment_tab;
mod avatar_tab;
mod badge_tab;
mod div_lab_tab;
mod gpui_elements_tab;
pub use div_lab_tab::DivLabTab;
pub use gpui_elements_tab::GpuiElementsTab;

mod bubble_tab;
mod button_tab;
mod calendar_tab;
mod carousel_tab;
mod chart_tab;
mod checkbox_tab;
mod clipboard_tab;
mod collapsible_tab;
mod color_picker_tab;
mod combobox_tab;
mod command_tab;
mod data_table_tab;
mod date_picker_tab;
mod description_list_tab;
mod dialog_tab;
mod dock_tab;
mod dropdown_button_tab;
mod editor_tab;
mod empty_tab;
mod form_tab;
mod gpui_form_tab;
mod group_box_tab;
mod hover_card_tab;
mod icon_tab;
mod image_tab;
mod input_group_tab;
mod input_tab;

mod kbd_tab;
mod label_tab;
mod list_tab;
mod marker_tab;
mod menu_tab;
mod message_scroller_tab;
mod message_tab;

mod notification_tab;
mod number_input_tab;
mod otp_input_tab;
mod pagination_tab;
mod popover_tab;
mod progress_tab;
mod questionnaire_tab;
mod radio_tab;
mod rating_tab;
mod resizable_tab;
mod scrollbar_tab;
mod select_tab;

mod settings_tab;
mod sheet_tab;

mod shimmer_tab;
mod sidebar_tab;
mod skeleton_tab;
mod slider_tab;
mod spinner_tab;
mod status_bar_tab;
mod stepper_tab;
mod switch_tab;
mod table_tab;
mod tabs_tab;
mod tag_tab;
mod textarea_tab;
mod theme_tab;
mod toggle_tab;
mod tooltip_tab;
mod tree_tab;
mod virtual_list_tab;

pub use accordion_tab::AccordionTab;
pub use alert_dialog_tab::AlertDialogTab;
pub use alert_tab::AlertTab;
pub use attachment_tab::AttachmentTab;
pub use avatar_tab::AvatarTab;
pub use badge_tab::BadgeTab;
pub use bubble_tab::BubbleTab;
pub use button_tab::ButtonTab;
pub use calendar_tab::CalendarTab;
pub use carousel_tab::CarouselTab;
pub use chart_tab::ChartTab;
pub use checkbox_tab::CheckboxTab;
pub use clipboard_tab::ClipboardTab;
pub use collapsible_tab::CollapsibleTab;
pub use color_picker_tab::ColorPickerTab;
pub use combobox_tab::ComboboxTab;
pub use command_tab::CommandTab;
pub use data_table_tab::DataTableTab;
pub use date_picker_tab::DatePickerTab;
pub use description_list_tab::DescriptionListTab;
pub use dialog_tab::DialogTab;
pub use dock_tab::DockTab;
pub use dropdown_button_tab::DropdownButtonTab;
pub use editor_tab::EditorTab;
pub use empty_tab::EmptyTab;
pub use form_tab::FormTab;
pub use gpui_form_tab::GpuiFormTab;
pub use group_box_tab::GroupBoxTab;
pub use hover_card_tab::HoverCardTab;
pub use icon_tab::IconTab;
pub use image_tab::ImageTab;
pub use input_group_tab::InputGroupTab;
pub use input_tab::InputTab;
pub use kbd_tab::KbdTab;
pub use label_tab::LabelTab;
pub use list_tab::ListTab;
pub use marker_tab::MarkerTab;
pub use menu_tab::MenuTab;
pub use message_scroller_tab::MessageScrollerTab;
pub use message_tab::MessageTab;
pub use notification_tab::NotificationTab;
pub use number_input_tab::NumberInputTab;
pub use otp_input_tab::OtpInputTab;
pub use pagination_tab::PaginationTab;
pub use popover_tab::PopoverTab;
pub use progress_tab::ProgressTab;
pub use questionnaire_tab::QuestionnaireTab;
pub use radio_tab::RadioTab;
pub use rating_tab::RatingTab;
pub use resizable_tab::ResizableTab;
pub use scrollbar_tab::ScrollbarTab;
pub use select_tab::SelectTab;
pub use settings_tab::SettingsTab;
pub use sheet_tab::SheetTab;
pub use shimmer_tab::ShimmerTab;
pub use sidebar_tab::SidebarTab;
pub use skeleton_tab::SkeletonTab;
pub use slider_tab::SliderTab;
pub use spinner_tab::SpinnerTab;
pub use status_bar_tab::StatusBarTab;
pub use stepper_tab::StepperTab;
pub use switch_tab::SwitchTab;
pub use table_tab::TableTab;
pub use tabs_tab::TabsTab;
pub use tag_tab::TagTab;
pub use textarea_tab::TextareaTab;
pub use theme_tab::ThemeColorsTab;
pub use toggle_tab::ToggleTab;
pub use tooltip_tab::TooltipTab;
pub use tree_tab::TreeTab;
pub use virtual_list_tab::VirtualListTab;

pub(crate) fn init(cx: &mut App) {
  input_tab::init(cx);
  combobox_tab::init(cx);
  rating_tab::init(cx);
  number_input_tab::init(cx);
  textarea_tab::init(cx);
  select_tab::init(cx);
  popover_tab::init(cx);
  menu_tab::init(cx);
  tooltip_tab::init(cx);
  otp_input_tab::init(cx);
  tree_tab::init(cx);
}

// Metadata stays alongside each page so individual modules remain self-describing.
#[allow(dead_code)]
pub trait ComponentPage: Render + Sized {
  fn klass() -> &'static str {
    std::any::type_name::<Self>().split("::").last().unwrap()
  }

  fn title() -> &'static str;

  fn description() -> &'static str {
    ""
  }

  fn closable() -> bool {
    true
  }

  fn zoomable() -> Option<PanelControl> {
    Some(PanelControl::default())
  }

  fn title_bg() -> Option<Hsla> {
    None
  }

  fn paddings() -> Pixels {
    px(16.)
  }

  fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;

  fn on_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
    let _ = active;
    let _ = window;
    let _ = cx;
  }

  fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App)
  where
    Self: 'static,
  {
    if let Some(story) = view.downcast::<Self>().ok() {
      cx.update_entity(&story, |story, cx| {
        story.on_active(active, window, cx);
      });
    }
  }
}

mod focus_trap_tab;
pub use focus_trap_tab::FocusTrapTab;
mod root_tab;
pub use root_tab::RootDemoTab;
mod plot_tab;
pub use plot_tab::PlotTab;
mod text_view_tab;
pub use text_view_tab::TextViewTab;
mod title_bar_tab;
pub use title_bar_tab::TitleBarTab;

mod support;
pub(crate) use support::{
  ChangeStorySize, TestAction, init_http, section, story_toolbar, story_toolbar_group, update_theme,
};
// Preserve the upstream questionnaire API for future examples, including unused entry points.
pub(crate) mod catalog;
pub(crate) mod components_tab;
#[allow(dead_code)]
mod questionnaire_base;
#[allow(dead_code, unused_imports)]
mod questionnaire_component;
