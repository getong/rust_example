use freya::router::Routable;

use crate::presentation::{
  counter::{CounterTab, StorageTab, SummaryTab},
  navigation::TabScaffold,
};

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub(crate) enum Route {
  #[layout(TabScaffold)]
    #[route("/", CounterTab)]
    Counter,
    #[route("/summary", SummaryTab)]
    Summary,
    #[route("/storage", StorageTab)]
    Storage,
}
