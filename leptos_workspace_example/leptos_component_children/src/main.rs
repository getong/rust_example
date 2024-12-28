use leptos::prelude::*;

// Often, you want to pass some kind of child view to another
// component. There are two basic patterns for doing this:
// - "render props": creating a component prop that takes a function that creates a view
// - the `children` prop: a special property that contains content passed as the children of a
//   component in your view, not as a property

#[component]
pub fn App() -> impl IntoView {
  let (items, set_items) = signal(vec![0, 1, 2]);
  let render_prop = move || {
    let len = move || items.read().len();
    view! { <p>"Length: " {len}</p> }
  };

  view! {
      // This component just displays the two kinds of children,
      // embedding them in some other markup
      // for component props, you can shorthand
      <TakesChildren // `render_prop=render_prop` => `render_prop`
      // (this doesn't work for HTML element attributes)
      render_prop>
          // these look just like the children of an HTML element
          <p>"Here's a child."</p>
          <p>"Here's another child."</p>
      </TakesChildren>
      <hr />
      // This component actually iterates over and wraps the children
      <WrapsChildren>
          <p>"Here's a child."</p>
          <p>"Here's another child."</p>
      </WrapsChildren>
  }
}

/// Displays a `render_prop` and some children within markup.
#[component]
pub fn TakesChildren<F, IV>(
  /// Takes a function (type F) that returns anything that can be
  /// converted into a View (type IV)
  render_prop: F,
  /// `children` takes the `Children` type
  /// this is an alias for `Box<dyn FnOnce() -> Fragment>`
  /// ... aren't you glad we named it `Children` instead?
  children: Children,
) -> impl IntoView
where
  F: Fn() -> IV,
  IV: IntoView,
{
  view! {
      <h1>
          <code>"<TakesChildren/>"</code>
      </h1>
      <h2>"Render Prop"</h2>
      {render_prop()}
      <hr />
      <h2>"Children"</h2>
      {children()}
  }
}

/// Wraps each child in an `<li>` and embeds them in a `<ul>`.
#[component]
pub fn WrapsChildren(children: ChildrenFragment) -> impl IntoView {
  // children() returns a `Fragment`, which has a
  // `nodes` field that contains a Vec<View>
  // this means we can iterate over the children
  // to create something new!
  let children = children()
    .nodes
    .into_iter()
    .map(|child| view! { <li>{child}</li> })
    .collect::<Vec<_>>();

  view! {
      <h1>
          <code>"<WrapsChildren/>"</code>
      </h1>
      // wrap our wrapped children in a UL
      <ul>{children}</ul>
  }
}

fn main() {
  leptos::mount::mount_to_body(App)
}
