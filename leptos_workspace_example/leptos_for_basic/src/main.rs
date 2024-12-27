use leptos::{mount::mount_to_body, prelude::*};

// Iteration is a very common task in most applications.
// So how do you take a list of data and render it in the DOM?
// This example will show you the two ways:
// 1) for mostly-static lists, using Rust iterators
// 2) for lists that grow, shrink, or move items, using <For/>

#[component]
fn App() -> impl IntoView {
  view! {
      <h1>"Iteration"</h1>
      <h2>"Static List"</h2>
      <p>"Use this pattern if the list itself is static."</p>
      <StaticList length=5 />
      <h2>"Dynamic List"</h2>
      <p>"Use this pattern if the rows in your list will change."</p>
      <DynamicList initial_length=5 />
  }
}

/// A list of counters, without the ability
/// to add or remove any.
#[component]
fn StaticList(
  /// How many counters to include in this list.
  length: usize,
) -> impl IntoView {
  // create counter signals that start at incrementing numbers
  let counters = (1 ..= length).map(|idx| RwSignal::new(idx));

  // when you have a list that doesn't change, you can
  // manipulate it using ordinary Rust iterators
  // and collect it into a Vec<_> to insert it into the DOM
  let counter_buttons = counters
    .map(|count| {
      view! {
          <li>
              <button on:click=move |_| *count.write() += 1>{count}</button>
          </li>
      }
    })
    .collect::<Vec<_>>();

  // Note that if `counter_buttons` were a reactive list
  // and its value changed, this would be very inefficient:
  // it would rerender every row every time the list changed.
  view! { <ul>{counter_buttons}</ul> }
}

/// A list of counters that allows you to add or
/// remove counters.
#[component]
fn DynamicList(
  /// The number of counters to begin with.
  initial_length: usize,
) -> impl IntoView {
  // This dynamic list will use the <For/> component.
  // <For/> is a keyed list. This means that each row
  // has a defined key. If the key does not change, the row
  // will not be re-rendered. When the list changes, only
  // the minimum number of changes will be made to the DOM.

  // `next_counter_id` will let us generate unique IDs
  // we do this by simply incrementing the ID by one
  // each time we create a counter
  let mut next_counter_id = initial_length;

  // we generate an initial list as in <StaticList/>
  // but this time we include the ID along with the signal
  // see NOTE in add_counter below re: ArcRwSignal
  let initial_counters = (0 .. initial_length)
    .map(|id| (id, ArcRwSignal::new(id + 1)))
    .collect::<Vec<_>>();

  // now we store that initial list in a signal
  // this way, we'll be able to modify the list over time,
  // adding and removing counters, and it will change reactively
  let (counters, set_counters) = signal(initial_counters);

  let add_counter = move |_| {
    // create a signal for the new counter
    // we use ArcRwSignal here, instead of RwSignal
    // ArcRwSignal is a reference-counted type, rather than the arena-allocated
    // signal types we've been using so far.
    // When we're creating a collection of signals like this, using ArcRwSignal
    // allows each signal to be deallocated when its row is removed.
    let sig = ArcRwSignal::new(next_counter_id + 1);
    // add this counter to the list of counters
    set_counters.update(move |counters| {
      // since `.update()` gives us `&mut T`
      // we can just use normal Vec methods like `push`
      counters.push((next_counter_id, sig))
    });
    // increment the ID so it's always unique
    next_counter_id += 1;
  };

  view! {
      <div>
          <button on:click=add_counter>"Add Counter"</button>
          <ul>
              // The <For/> component is central here
              // This allows for efficient, key list rendering
              <For
                  // `each` takes any function that returns an iterator
                  // this should usually be a signal or derived signal
                  // if it's not reactive, just render a Vec<_> instead of <For/>
                  each=move || counters.get()
                  // the key should be unique and stable for each row
                  // using an index is usually a bad idea, unless your list
                  // can only grow, because moving items around inside the list
                  // means their indices will change and they will all rerender
                  key=|counter| counter.0
                  // `children` receives each item from your `each` iterator
                  // and returns a view
                  children=move |(id, count)| {
                      let count = RwSignal::from(count);
                      // we can convert our ArcRwSignal to a Copy-able RwSignal
                      // for nicer DX when moving it into the view
                      view! {
                          <li>
                              <button on:click=move |_| *count.write() += 1>{count}</button>
                              <button on:click=move |_| {
                                  set_counters
                                      .write()
                                      .retain(|(counter_id, _)| { counter_id != &id });
                              }>"Remove"</button>
                          </li>
                      }
                  }
              />
          </ul>
      </div>
  }
}

fn main() {
  mount_to_body(App);
}
