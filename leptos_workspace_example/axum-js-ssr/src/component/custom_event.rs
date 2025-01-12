use leptos::prelude::*;
use leptos_meta::Script;

use crate::{app::*, consts::LEPTOS_HYDRATED};

#[component]
pub fn CustomEvent() -> impl IntoView {
  let js_hook = format!(
    "\
        var events = [];
if (!window.hljs) {{
    console.log('pushing listener for hljs load');
    events.push(new Promise((r) =>
        document.querySelector('#hljs-src').addEventListener('load', r, false)));
}}
if (!window.{LEPTOS_HYDRATED}) {{
    console.log('pushing listener for leptos hydration');
    events.push(new Promise((r) => document.addEventListener('{LEPTOS_HYDRATED}', r, false)));
}}
Promise.all(events).then(() => {{
    console.log(`${{events.length}} events have been dispatched; now calling highlightAll()`);
    hljs.highlightAll();
}});
"
  );
  provide_context(CodeDemoHook {
    js_hook: js_hook.clone(),
  });
  // FIXME Seems like <Script> require a text node, otherwise hydration error from marker mismatch
  view! {
      <h2>"Have Leptos dispatch an event when body is hydrated"</h2>
      <CodeDemo />
      <Script id="hljs-src" async_="true" src="/highlight.min.js">
          ""
      </Script>
      <p>
          "
             So if using events fixes problems with timing issues, couldn't Leptos provide an event to signal
             that the body is hydrated?  Well, this problem is typically solved by having a signal in the
             component, and then inside the "<code>"Suspend"</code>" provide an "
          <code>"Effect"</code>" that
          would set the signal to "<code>"Some"</code>" string that will then mount the "
          <code>"<Script>"</code>
          " onto the body.  However, if a hydrated event is desired from within JavaScript (e.g.
            where some existing JavaScript library/framework is managing event listeners for some particular
            reason), given that typical Leptos applications provide the "<code>"fn hydate()"</code>
          " (usually
          in "<code>" lib.rs"</code>"), that can be achieved by providing the following after "
          <code>"leptos::mount::hydrate_body(App);"</code>".
          "
      </p>
      <div>
          <pre>
              <code class="language-rust">
                  {format!(
                      r#"#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {{
    use app::App;
    // ... other calls omitted, as this example is only a rough
    // reproduction of what is actually executed.
    leptos::mount::hydrate_body(App);

    // Now hydrate_body is done, provide ways to inform that
    let window = leptos::prelude::window();
    // first set a flag to signal that hydration has happened and other
    // JavaScript code may just run without waiting for the event that
    // is just about to be dispatched, as the event is only a one-time
    // deal but this lives on as a variable that can be checked.
    js_sys::Reflect::set(
        &window,
        &wasm_bindgen::JsValue::from_str({LEPTOS_HYDRATED:?}),
        &wasm_bindgen::JsValue::TRUE,
    ).expect("error setting hydrated status");
    // Then dispatch the event for all the listeners that were added.
    let event = web_sys::Event::new({LEPTOS_HYDRATED:?})
        .expect("error creating hydrated event");
    let document = leptos::prelude::document();
    document.dispatch_event(&event)
        .expect("error dispatching hydrated event");
}}"#,
                  )}
              </code>
          </pre>
      </div>
      <p>
          "
             With the notification that hydration is completed, the following JavaScript code may be called
             inside "<code>"Suspense"</code>
          " block (in this live example, it's triggered by providing the
          following JavaScript code via a "<code>"provide_context"</code> " which the code rendering
          component will then use within a "<code>"Suspend"</code>"):
          "
      </p>
      <div>
          <pre>
              <code class="language-javascript">{js_hook}</code>
          </pre>
      </div>
      <p>
          "
          For this simple example with a single "<code>"Suspense"</code>
          ", no matter what latency there is,
          in whichever order the API calls are completed, the setup ensures that "
          <code>"highlightAll()"</code>
          " is called only after hydration is done and also after the delayed content is properly
            rendered onto the DOM.  Specifically, only use the event to wait for the required resource if it
            is not set to a ready state, and wait for all the events to become ready before actually calling
            the function.
            "
      </p>
      <p>
          "
          If there are multiple "<code>"Suspense"</code>
          ", it will be a matter of adding all the event
          listeners that will respond to the completion of all the "<code>"Suspend"</code>
          "ed futures, which
            will then invoke the code highlighting function.
            "
      </p>
  }
}
