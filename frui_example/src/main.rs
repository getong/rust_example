#![feature(min_specialization)]
#![feature(type_alias_impl_trait)]

use frui::prelude::*;

#[derive(ViewWidget)]
struct App;

impl ViewWidget for App {
    fn build<'w>(&'w self, _: BuildContext<'w, Self>) -> Self::Widget<'w> {
        Center::child(Text::new("Hello, World!"))
    }
}

fn main() {
    run_app(App);
}
