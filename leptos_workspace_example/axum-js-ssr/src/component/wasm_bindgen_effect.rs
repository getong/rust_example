use leptos::prelude::*;

use crate::app::*;

#[component]
pub fn WasmBindgenEffect() -> impl IntoView {
  let example = r#"<Suspense fallback=move || view! { <p>"Loading code example..."</p> }>{
    move || Suspend::new(async move {
        Effect::new(move |_| {
            request_animation_frame(move || {
                leptos::logging::log!("request_animation_frame invoking hljs::highlight_all");
                // under SSR this is an noop.
                crate::hljs::highlight_all();
            });
        });
        view! {
            <pre><code>{code.await}</code></pre>
        }
    })
}</Suspense>"#;

  view! {
      <h2>"Using "<code>"wasm-bindgen"</code>" with proper consideration, part 2"</h2>
      <CodeDemoWasm mode=WasmDemo::RequestAnimationFrame />
      <p>
          "
          This example simply uses "<code>"window.requestAnimationFrame()"</code> " (via the binding
          available as "<code>"leptos::prelude::request_animation_frame"</code>
          ") to delay the running of
            the highlighting by a tick so that both the hydration would complete for SSR, and that it would
            also delay highlighting call to after the suspend results are loaded onto the DOM.  The Suspend
            for the dynamic code block is simply reduced to the following:
            "
      </p>
      <div>
          <pre>
              <code class="language-rust">{example}</code>
          </pre>
      </div>
      <p>
          "
             However, this method does have a drawback, which is that the inline code blocks will be processed
             multiple times by this indiscriminate method (which "<code>"highlight.js"</code>
          " thankfully has a
            failsafe detection which avoids issues, but definitely don't count on this being the norm with
            JavaScript libraries).  We could go back to the previous example where we use events to trigger
            for when the Suspend is resolved, but this will mean there needs to be some way to co-ordinate and
            wait for all of them to ensure the JavaScript library is only invoked once on the hydrated output.
            "
      </p>
      <p>
          "
             If the JavaScript library provides an alternative API that does not involve this wrestling of the
             DOM but does achieve the intended objectives is in fact available, it would definitely be the
             better choice.  Even better, make them available in Rust through "
          <code>"wasm-bindgen"</code>" so
          that the relevant Leptos component may use them directly.  In the next couple examples we will see
          how this idea may be put into practice.
          "
      </p>
  }
}
