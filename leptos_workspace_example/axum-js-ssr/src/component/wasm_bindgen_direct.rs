use leptos::prelude::*;

use crate::{app::*, component::wasm_bindgen_direct_fixed::CodeDemoWasmInner};

#[component]
pub fn WasmBindgenDirect() -> impl IntoView {
  let code = r#"#[component]
fn CodeInner(code: String, lang: String) -> impl IntoView {
    #[cfg(feature = "ssr")]
    let inner = Some(html_escape::encode_text(&code).into_owned());
    #[cfg(not(feature = "ssr"))]
    let inner = crate::hljs::highlight(code, lang);
    view! {
        <pre><code inner_html=inner></code></pre>
    }
}

// Simply use the above component in a view like so:
//
// view! { <CodeInner code lang/> }"#
    .to_string();
  let lang = "rust".to_string();

  view! {
      <h2>"If possible, avoid DOM manipulation outside of Leptos"</h2>
      <CodeDemoWasmInner />
      <p>
          "
             Whenever possible, look for a way to use the target JavaScript library to produce the desired
             markup without going through a global DOM manipulation can end up being much more straight-forward
             to write when working in pure Rust code.  More so if there is a server side counterpart, which
             means the use of the module don't need the disambiguation within the component itself.  A
             simplified version of a component that will render a code block that gets highlighted under CSR
             (and plain text under SSR) may look something like this:
            "
      </p>
      <CodeInner code lang />
      <p>
          "
          In the above example, no additional "<code>"<script>"</code>
          " tags, post-hydration processing,
            event listeners nor other DOM manipuation are needed, as the JavaScript function that converts a
            string to highlighted markup can be made from Rust through bindings generated with the use of
            "<code>"wasm-bindgen"</code>
          " under CSR.  As the highlight functionality isn't available under
          SSR, the incoming code is simply processed using " <code>"html_escape::encode_text"</code>
          ".
          "
      </p>
      <p>
          "
             ... Well, if only it actually works, as there is a bit of an unexpected surprise during hydration.
             During the hydration of the above code rendering component, the CSR specific pipeline kicks in and
             calls "<code>"hljs::highlight"</code>
          ", producing a different output that was assumed to trigger
            a re-rendering.  As hydration assumes the HTML rendered under SSR is isomorphic with CSR, a
            violation of this expectation (i.e. CSR rendering something entierly different) is not something
            it anticipates; the lack of re-rendering is in fact an optimization for performance reasons as it
            avoids unnecessary work.  However in this instance, that isn't the desired behavior as the the
            syntax highlighting will not be shown as expected, and thankfully in this instance it does not
            result in a crash.
            "
      </p>
      <p>
          "
             All that being said, the code is not doing what is desired, is there any way to go about this?
             Fortunately, this is where effects comes in as it provides the intent to do something on the
             client side, being able to function as an opt-in for CSR content to \"overwrite\" SSR content.
             The next and final example will show how this should be done.
            "
      </p>
  }
}
