use leptos::prelude::*;
use leptos_meta::*;

use crate::app::{CodeDemo, CodeDemoHook};

#[component]
pub fn NaiveEvent(#[prop(optional)] hook: bool, #[prop(optional)] fallback: bool) -> impl IntoView {
  let render_hook = "\
document.querySelector('#hljs-src')
    .addEventListener('load', (e) => { hljs.highlightAll() }, false);";
  let render_call = "\
if (window.hljs) {
    hljs.highlightAll();
} else {
    document.querySelector('#hljs-src')
        .addEventListener('load', (e) => { hljs.highlightAll() }, false);
}";
  let js_hook = if fallback { render_call } else { render_hook };
  let explanation = if hook {
    provide_context(CodeDemoHook {
      js_hook: js_hook.to_string(),
    });
    if fallback {
      view! {
          <ol>
              <li>
                  "
                  In this iteration, the following load hook is set in a " <code>"<Script>"</code>"
                  component after the dynamically loaded code example." <pre>
                      <code class="language-javascript">{js_hook}</code>
                  </pre>
              </li>
              <li>
                  <strong>CSR</strong>
                  "
                  This works much better now under CSR due to the fallback that checks whether the
                  library is already loaded or not.  Using the library directly if it's already loaded
                  and only register the event otherwise solves the rendering issue under CSR.
                  "
              </li>
              <li>
                  <strong>SSR</strong>
                  "
                  Much like the second example, hydration will still panic some of the time as per the
                  race condition that was described.
                  "
              </li>
          </ol>
          <p>
              "
               All that being said, all these naive examples still result in hydration being
               non-functional in varying degrees of (non-)reproducibility due to race conditions.  Is
               there any way to fix this?  Is "<code>"wasm-bindgen"</code>
              " the only answer?  What if the
              goal is to incorporate external scripts that change often and thus can't easily have
              bindings built?  Follow onto the next examples to solve some of this, at the very least
              prevent the panic during hydration.
              "
          </p>
      }.into_any()
    } else {
      view! {
          <ol>
              <li>
                  "
                  In this iteration, the following load hook is set in a " <code>"<Script>"</code>"
                  component after the dynamically loaded code example." <pre>
                      <code class="language-javascript">{js_hook}</code>
                  </pre>
              </li>
              <li>
                  <strong>CSR</strong>
                  "
                  Unfortunately, this still doesn't work reliably to highlight both code examples, in
                  fact, none of the code examples may highlight at all!  Placing the JavaScript loader
                  hook inside a "
                  <code>Suspend</code>
                  " will significantly increase the likelihood that
                  the event will be fired long before the loader adds the event hook.  As a matter of
                  fact, the highlighting is likely to only work with the largest latencies added for
                  the loading of "
                  <code>"highlight.js"</code>
                  ", but at least both code examples will
                  highlight when working.
                  "
              </li>
              <li>
                  <strong>SSR</strong>
                  "
                  Much like the second example, hydration will still panic some of the time as per the
                  race condition that was described - basically if the timing results in CSR not showing
                  highlight code, the code will highlight here in SSR but will panic during hydration.
                  "
              </li>
          </ol>
      }.into_any()
    }
  } else {
    view! {
        <ol>
            <li>
                "
                In this iteration, the following hook is set in a "<code>"<Script>"</code>
                " component
                immediately following the one that loaded "<code>"highlight.js"</code>".
                "<pre>
                    <code class="language-javascript">{js_hook}</code>
                </pre>
            </li>
            <li>
                <strong>CSR</strong>
                "
                Unfortunately, the hook is being set directly on this component, rather than inside the
                view for the dynamic block.  Given the nature of asynchronous loading which results in the
                uncertainty of the order of events, it may or may not result in the dynamic code block (or
                any) being highlighted under CSR (as there may or may not be a fully formed code block for
                highlighting to happen).  This is affected by latency, so the loader here emulates a small
                number of latency values (they repeat in a cycle).  The latency value is logged into the
                console and it may be referred to witness its effects on what it does under CSR - look for
                the line that might say \"loaded standard highlight.js with a minimum latency of 40 ms\".
                Test this by going from home to here and then navigating between them using the browser's
                back and forward feature for convenience - do ensure the "
                <code>"highlight.js"</code>
                "
                isn't being cached by the browser.
                "
            </li>
            <li>
                <strong>SSR</strong>
                "
                Moreover, hydration will panic if the highlight script is loaded before hydration is
                completed (from the resulting DOM mismatch after code highlighting).  Refreshing here
                repeatedly may trigger the panic only some of the time when the "
                <code>"highlight.js"</code>
                " script is loaded under the lowest amounts of artificial delay, as even under no
                latency the hydration can still succeed due to the non-deterministic nature of this race
                condition.
                "
            </li>
        </ol>
    }.into_any()
  };
  // FIXME Seems like <Script> require a text node, otherwise hydration error from marker mismatch
  view! {
      <h2>"Using the Leptos "<code>"<Script>"</code>" component asynchronously instead"</h2>
      <CodeDemo />
      <Script id="hljs-src" async_="true" src="/highlight.min.js">
          ""
      </Script>
      // Example 2's <Script> invocation; Example 3 and 4 will be provided via a context to allow the
      // inclusion of the `highlightAll()` call in the Suspend
      {(!hook).then(|| view! { <Script>{render_hook}</Script> })}
      <p>
          "
           What the "<code>"<Script>"</code>" component does is to ensure the "
          <code>"<script>"</code>" tag
          is placed in the document head in the order it is defined in a given component, rather than at
          where it was placed into the DOM.  Note that it is also a reactive component, much like the first
          example, it gets unloaded under CSR when the component is no longer active, In this improved
          version, "<code>"highlight.js"</code>" is also loaded asynchronously (using the "
          <code>"async"</code>
          " attribute), to allow an event listener that can delay highlighting to after the library
            is loaded.  This should all work out fine, right?
            "
      </p>
      {explanation}
  }
}
