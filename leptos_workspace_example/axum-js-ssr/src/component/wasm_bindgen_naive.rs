use leptos::prelude::*;

use crate::app::*;

#[component]
pub fn WasmBindgenNaive() -> impl IntoView {
  let example = r#"<Suspense fallback=move || view! { <p>"Loading code example..."</p> }>{
    move || Suspend::new(async move {
        view! {
            <pre><code>{code.await}</code></pre>
            {
                #[cfg(not(feature = "ssr"))]
                {
                    use crate::hljs::highlight_all;
                    leptos::logging::log!("calling highlight_all");
                    highlight_all();
                }
            }
        }
    })
}</Suspense>"#;
  view! {
      <h2>"Will "<code>"wasm-bindgen"</code>" magically avoid all the problems?"</h2>
      <CodeDemoWasm mode=WasmDemo::Naive />
      <p>
          "
             Well, the naively done example clearly does not work, as the behavior of this demo is almost
             exactly like the very first naive JavaScript example (after the script loaded), where only the
             inline code block will highlight under CSR and hydration is broken when trying to load this under
             SSR.  This is the consequence of porting the logic naively.  In this example, the calling of
             "<code>"hljs::highlight_all()"</code>" is located inside a "<code>"Suspend"</code>
          " immediately
            after the code block, but it doesn't mean the execution will apply to that because it hasn't been
            mounted onto the DOM itself for "<code>"highlight.js"</code>" to process.
          "
      </p>
      <p>
          "
             Similarly, SSR may also error under a similar mechanism, which again breaks hydration because the
             code is run on the dehydrated nodes before hydration has happened.  Using event listeners via
             "<code>"web_sys"</code>
          " in a similar manner like the JavaScript based solutions shown previously
            can fix this, but there are other approaches also.
            "
      </p>
      <p>
          "
          For a quick reference, the following is the "<code>"Suspense"</code>
          " that would ultimately render
            the dynamic code block:
            "
      </p>
      <div>
          <pre>
              <code class="language-rust">{example}</code>
          </pre>
      </div>
  }
}
