use leptos::prelude::*;

use crate::app::*;

#[component]
pub fn WasmBindgenJSHookReadyEvent() -> impl IntoView {
  let example = r#"#[cfg(not(feature = "ssr"))]
{
    use crate::hljs;
    use wasm_bindgen::{closure::Closure, JsCast};

    let document = document();
    // Rules relating to hydration still applies when loading via SSR!  Changing
    // the dom before hydration is done is still problematic, as the same issues
    // such as the panic as demonstrated in the relevant JavaScript demo.
    let hydrate_listener = Closure::<dyn Fn(_)>::new(move |_: web_sys::Event| {
        leptos::logging::log!("wasm hydration_listener highlighting");
        hljs::highlight_all();
    }).into_js_value();
    document.add_event_listener_with_callback(
        LEPTOS_HYDRATED,
        hydrate_listener.as_ref().unchecked_ref(),
    ).expect("failed to add event listener to document");

    // For CSR rendering, wait for the hljs_hook which will be fired when this
    // suspended bit is fully mounted onto the DOM, and this is done using a
    // JavaScript shim described below.
    let csr_listener = Closure::<dyn FnMut(_)>::new(move |_: web_sys::Event| {
        leptos::logging::log!("wasm csr_listener highlighting");
        hljs::highlight_all();
    }).into_js_value();
    let options = web_sys::AddEventListenerOptions::new();
    options.set_once(true);
    // FIXME this actually is not added as a unique function so after a quick re-
    // render will re-add this as a new listener, which causes a double call
    // to highlightAll.  To fix this there needs to be a way to put the listener
    // and keep it unique, but this looks to be rather annoying to do from within
    // this example...
    document.add_event_listener_with_callback_and_add_event_listener_options(
        "hljs_hook",
        csr_listener.as_ref().unchecked_ref(),
        &options,
    ).expect("failed to add event listener to document");
    leptos::logging::log!("wasm csr_listener listener added");

    // Dispatch the event when this view is finally mounted onto the DOM.
    request_animation_frame(move || {
        let event = web_sys::Event::new("hljs_hook")
            .expect("error creating hljs_hook event");
        document.dispatch_event(&event)
            .expect("error dispatching hydrated event");
    });
    // Alternative, use a script tag, but at that point, you might as well write
    // all of the above in JavaScript because in this simple example none of the
    // above is native to Rust or Leptos.
}"#;

  view! {
      <h2>"Using "<code>"wasm-bindgen"</code>" with proper consideration"</h2>
      <CodeDemoWasm mode=WasmDemo::ReadyEvent />
      <p>
          "
             Well, this works a lot better, under SSR the code is highlighted only after hydration to avoid the
             panic, and under CSR a new event is created for listening and responding to for the rendering to
             happen only after the suspended node is populated onto the DOM.  There is a bit of a kink with the
             way this is implemented, but it largely works.
            "
      </p>
      <p>
          "
             The code that drives this is needlessly overcomplicated, to say the least.  This is what got added
             to the "<code>"view! {...}"</code>" from the last example:
          "
      </p>
      <details>
          <summary>"Expand for the rather verbose code example"</summary>
          <div>
              <pre>
                  <code class="language-rust">{example}</code>
              </pre>
          </div>
      </details>
      <p>
          "
             Given that multiple frameworks that will manipulate the DOM in their own and assume they are the
             only source of truth is the problem - being demonstrated by Leptos in previous examples assuming
             that nothing else would change the DOM for hydration.  So if it is possible to use the JavaScript
             library in a way that wouldn't cause unexpected DOM changes, then that can be a way to avoid
             needing all these additional event listeners for working around the panics.
            "
      </p>
      <p>
          "
             One thing to note is that this is a very simple example with a single Suspense (or Transition), so
             if there are more than one of them and they have significantly different resolution timings,
             calling that potentially indiscriminate JavaScript DOM manipulation function may require
             additional care (e.g. needing to wait for all the events in a future before making the final call
             to do make the invasive DOM manipulation).  Let's look at one more similar example that use a
             cheap workaround that may work for cases like integrating the simple JavaScript library here.
            "
      </p>
  }
}
