use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
  Widgets,
  Layout,
  About,
}

struct BasicEguiApp {
  page: Page,
  name: String,
  enabled: bool,
  volume: f32,
  choice: usize,
  color: egui::Color32,
  show_window: bool,
  click_count: u32,
  status: String,
}

impl Default for BasicEguiApp {
  fn default() -> Self {
    Self {
      page: Page::Widgets,
      name: "egui 学习者".to_owned(),
      enabled: true,
      volume: 0.65,
      choice: 0,
      color: egui::Color32::from_rgb(80, 160, 220),
      show_window: false,
      click_count: 0,
      status: "等待操作".to_owned(),
    }
  }
}

impl eframe::App for BasicEguiApp {
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    let ctx = ui.ctx().clone();

    egui::Panel::top("top_bar").show(ui, |ui| {
      egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("示例", |ui| {
          if ui.button("重置控件状态").clicked() {
            *self = Self::default();
            ui.close();
          }
          if ui.button("退出窗口").clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          }
        });
        ui.separator();
        ui.label(egui::RichText::new("egui 基本用法").strong());
      });
    });

    egui::Panel::left("navigation")
      .default_size(160.0)
      .show(ui, |ui| {
        ui.heading("学习导航");
        ui.add_space(8.0);
        ui.selectable_value(&mut self.page, Page::Widgets, "控件与状态");
        ui.selectable_value(&mut self.page, Page::Layout, "布局与容器");
        ui.selectable_value(&mut self.page, Page::About, "egui 的特点");
        ui.add_space(16.0);
        ui.separator();
        ui.small("左侧切换页面，右侧直接操作示例。");
      });

    egui::CentralPanel::default().show(ui, |ui| match self.page {
      Page::Widgets => self.widgets_page(ui),
      Page::Layout => self.layout_page(ui),
      Page::About => self.about_page(ui),
    });

    if self.show_window {
      egui::Window::new("独立窗口")
        .open(&mut self.show_window)
        .resizable(true)
        .show(&ctx, |ui| {
          ui.label("Window 是可以叠加在面板上的容器。");
          ui.label(format!("当前点击次数：{}", self.click_count));
        });
    }
  }
}

impl BasicEguiApp {
  fn widgets_page(&mut self, ui: &mut egui::Ui) {
    ui.heading("常用控件");
    ui.label("每一帧都根据当前状态重新描述界面，控件的返回值负责告诉我们发生了什么。");
    ui.add_space(12.0);

    egui::Grid::new("widget_grid")
      .num_columns(2)
      .spacing([24.0, 10.0])
      .show(ui, |ui| {
        ui.label("文本输入");
        ui.add(egui::TextEdit::singleline(&mut self.name).hint_text("请输入名称"));
        ui.end_row();

        ui.label("复选框");
        ui.checkbox(&mut self.enabled, "启用功能");
        ui.end_row();

        ui.label("单选按钮");
        ui.horizontal(|ui| {
          ui.radio_value(&mut self.choice, 0, "选项 A");
          ui.radio_value(&mut self.choice, 1, "选项 B");
        });
        ui.end_row();

        ui.label("下拉框");
        egui::ComboBox::from_id_salt("choice_combo")
          .selected_text(format!("选项 {}", if self.choice == 0 { "A" } else { "B" }))
          .show_ui(ui, |ui| {
            ui.selectable_value(&mut self.choice, 0, "选项 A");
            ui.selectable_value(&mut self.choice, 1, "选项 B");
          });
        ui.end_row();

        ui.label("滑块");
        ui.add(egui::Slider::new(&mut self.volume, 0.0 ..= 1.0).text("音量"));
        ui.end_row();

        ui.label("颜色选择");
        ui.color_edit_button_srgba(&mut self.color);
        ui.end_row();
      });

    ui.separator();
    ui.horizontal(|ui| {
      if ui.button("点击我").clicked() {
        self.click_count += 1;
        self.status = format!("按钮已点击 {} 次", self.click_count);
      }
      if ui.button("打开独立窗口").clicked() {
        self.show_window = true;
      }
      ui.label(&self.status);
    });
    ui.add(egui::ProgressBar::new(self.volume).show_percentage());
  }

  fn layout_page(&mut self, ui: &mut egui::Ui) {
    ui.heading("布局与容器");
    ui.label("面板、水平/垂直布局、滚动区域和折叠标题都是组合 UI 的基础。");
    ui.add_space(8.0);

    egui::CollapsingHeader::new("水平布局：horizontal")
      .default_open(true)
      .show(ui, |ui| {
        ui.horizontal(|ui| {
          ui.label("同一行");
          let _ = ui.button("按钮 1");
          let _ = ui.button("按钮 2");
        });
      });

    egui::CollapsingHeader::new("滚动区域：ScrollArea")
      .default_open(true)
      .show(ui, |ui| {
        egui::ScrollArea::vertical()
          .max_height(150.0)
          .show(ui, |ui| {
            for index in 1 ..= 12 {
              ui.label(format!("滚动内容第 {index} 行"));
            }
          });
      });

    egui::Frame::group(ui.style()).show(ui, |ui| {
      ui.label(egui::RichText::new("Frame 可以给一组控件添加边框和背景。").strong());
      ui.label("CentralPanel 通常最后绘制；SidePanel 和 TopBottomPanel 用于搭建整体结构。");
    });
  }

  fn about_page(&mut self, ui: &mut egui::Ui) {
    ui.heading("egui 的特点");
    ui.add_space(8.0);
    let features = [
      (
        "即时模式 GUI",
        "每帧执行 UI 代码，根据当前状态生成界面，不需要手动维护复杂的控件树。",
      ),
      (
        "Rust 原生",
        "类型安全、依赖少，适合工具、调试面板、编辑器和跨平台桌面应用。",
      ),
      (
        "布局组合简单",
        "通过闭包组合 Panel、Window、ScrollArea 和各种 Widget，代码结构直观。",
      ),
      (
        "响应式交互",
        "按钮、输入框等控件返回 Response，可用 clicked、changed 等方法处理事件。",
      ),
      (
        "跨平台渲染",
        "egui 负责 UI 描述，eframe 提供窗口、事件循环和渲染后端的集成。",
      ),
    ];
    for (title, description) in features {
      ui.horizontal_wrapped(|ui| {
        ui.label(
          egui::RichText::new(title)
            .strong()
            .color(egui::Color32::LIGHT_BLUE),
        );
        ui.label(description);
      });
      ui.add_space(6.0);
    }
    ui.separator();
    ui.label("建议学习顺序：Ui/Widget -> Response -> Panel/Window -> 状态管理 -> 自定义绘制。 ");
  }
}

fn main() -> eframe::Result<()> {
  let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_inner_size([900.0, 620.0])
      .with_min_inner_size([640.0, 420.0]),
    ..Default::default()
  };

  eframe::run_native(
    "egui 基本用法",
    options,
    Box::new(|_creation_context| Ok(Box::new(BasicEguiApp::default()))),
  )
}
