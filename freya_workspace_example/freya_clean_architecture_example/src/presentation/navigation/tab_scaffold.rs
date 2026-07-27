use freya::{
  icons::lucide,
  prelude::*,
  router::{Outlet, RouterContext, use_route},
};

use crate::presentation::navigation::Route;

#[derive(PartialEq)]
pub(crate) struct TabScaffold;

impl Component for TabScaffold {
  fn render(&self) -> impl IntoElement {
    rect()
      .native_router()
      .expanded()
      .content(Content::flex())
      .background((244, 246, 249))
      .color((35, 39, 47))
      .child(
        rect()
          .width(Size::fill())
          .height(Size::flex(1.))
          .padding((0., 0., 16., 0.))
          .overflow(Overflow::Clip)
          .child(Outlet::<Route>::new()),
      )
      .child(
        rect()
          .horizontal()
          .width(Size::fill())
          .height(Size::px(76.))
          .padding(6.)
          .spacing(4.)
          .background((255, 255, 255))
          .child(TabItem::new(Route::Counter, "Counter", TabIcon::Counter))
          .child(TabItem::new(Route::Summary, "Summary", TabIcon::Summary))
          .child(TabItem::new(Route::Storage, "Storage", TabIcon::Storage)),
      )
  }
}

#[derive(Clone, Copy, PartialEq)]
enum TabIcon {
  Counter,
  Summary,
  Storage,
}

#[derive(PartialEq)]
struct TabItem {
  route: Route,
  label: &'static str,
  icon: TabIcon,
}

impl TabItem {
  const fn new(route: Route, label: &'static str, icon: TabIcon) -> Self {
    Self { route, label, icon }
  }
}

impl Component for TabItem {
  fn render(&self) -> impl IntoElement {
    let is_active = use_route::<Route>() == self.route;
    let route = self.route.clone();
    let foreground = if is_active {
      (15, 112, 190)
    } else {
      (92, 99, 112)
    };
    let background = if is_active {
      (225, 241, 252)
    } else {
      (255, 255, 255)
    };
    let icon = match self.icon {
      TabIcon::Counter => lucide::sliders_horizontal(),
      TabIcon::Summary => lucide::notebook_text(),
      TabIcon::Storage => lucide::settings(),
    };

    rect()
      .width(Size::percent(32.))
      .height(Size::fill())
      .center()
      .spacing(3.)
      .corner_radius(6.)
      .background(background)
      .color(foreground)
      .on_press(move |_| {
        let _ = RouterContext::get().replace(route.clone());
      })
      .child(
        SvgViewer::new(icon)
          .color(foreground)
          .width(Size::px(20.))
          .height(Size::px(20.)),
      )
      .child(self.label)
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use freya::{
    radio::use_init_radio_station,
    router::{Router, RouterConfig},
  };
  use freya_testing::TestingRunner;

  use super::*;
  use crate::{
    domain::{
      entities::{Counter, CounterId},
      repositories::{CounterRepository, CounterRepositoryError},
    },
    presentation::counter::{CounterChannel, CounterState},
  };

  struct TestCounterRepository;

  impl CounterRepository for TestCounterRepository {
    fn load(&self) -> Result<Option<Counter>, CounterRepositoryError> {
      Ok(Some(Counter::new(CounterId::new(1), 4)))
    }

    fn save(&self, _counter: &Counter) -> Result<(), CounterRepositoryError> {
      Ok(())
    }
  }

  fn test_app() -> impl IntoElement {
    use_init_radio_station::<CounterState, CounterChannel>(|| {
      CounterState::restore(
        Counter::new(CounterId::new(1), 4),
        Rc::new(TestCounterRepository),
        "counter.json".to_string(),
      )
    });

    Router::<Route>::new(|| RouterConfig::default().with_initial_path(Route::Counter))
  }

  fn label_vertical_bounds(test: &TestingRunner, text: &str) -> (f32, f32) {
    test
      .find(|node, element| {
        Label::try_downcast(element)
          .filter(|label| label.text.as_ref() == text)
          .map(|_| {
            let area = node.layout().area;
            (area.min_y(), area.max_y())
          })
      })
      .unwrap_or_else(|| panic!("label {text:?} should be rendered"))
  }

  fn action_bar_bottom(test: &TestingRunner) -> f32 {
    test
      .find(|node, element| {
        Rect::try_downcast(element).and_then(|_| {
          let area = node.layout().area;
          ((area.height() - 60.).abs() < 0.5 && area.min_y() > 300.).then_some(area.max_y())
        })
      })
      .expect("the current page should render a 60px action bar")
  }

  fn bottom_navigation_top(test: &TestingRunner) -> f32 {
    test
      .find(|node, element| {
        Rect::try_downcast(element).and_then(|_| {
          let area = node.layout().area;
          (area.width() > 500. && area.min_y() > 500.).then_some(area.min_y())
        })
      })
      .expect("the bottom navigation should be rendered")
  }

  fn assert_action_bar_above_bottom_navigation(test: &TestingRunner, page: &str) {
    let action_bottom = action_bar_bottom(test);
    let navigation_top = bottom_navigation_top(test);

    assert!(
      action_bottom + 16. <= navigation_top,
      "{page} action bar ends at {action_bottom}, bottom navigation starts at {navigation_top}"
    );
  }

  #[test]
  fn tab_actions_are_visible_above_the_bottom_navigation() {
    let (mut test, _) = TestingRunner::new(test_app, (520., 680.).into(), |_| {}, 1.);

    label_vertical_bounds(&test, "Increase");
    label_vertical_bounds(&test, "Decrease");
    assert_action_bar_above_bottom_navigation(&test, "Counter");

    test.click_cursor((260., 640.));
    label_vertical_bounds(&test, "Distance to zero");
    label_vertical_bounds(&test, "Increase");
    label_vertical_bounds(&test, "Decrease");
    assert_action_bar_above_bottom_navigation(&test, "Summary");

    test.click_cursor((430., 640.));
    label_vertical_bounds(&test, "Save now");
    assert_action_bar_above_bottom_navigation(&test, "Storage");
  }
}
