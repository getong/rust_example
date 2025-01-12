use chrono::*;
use leptos::prelude::*;
use thaw::{Calendar, Space};

#[component]
pub fn CalendarElement() -> impl IntoView {
  let value = RwSignal::new(Local::now().date_naive());
  let option_value = RwSignal::new(Some(Local::now().date_naive()));

  view! {
      <Space vertical=true>
          <Calendar value />
          <Calendar value=option_value let(date: &NaiveDate)>
              {date.year()}
              "-"
              {date.month()}
              "-"
              {date.day()}
          </Calendar>
      </Space>
      <p>
          <a href="/">Back to Home</a>
      </p>
  }
}
