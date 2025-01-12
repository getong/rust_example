use leptos::prelude::*;

use crate::{api::fetch_code, app::*, consts::CH03_05A};

#[component]
pub fn WasmBindgenDirectFixed() -> impl IntoView {
  let code = r#"#[component]
fn CodeInner(code: String, lang: String) -> impl IntoView {
    let (inner, set_inner) = signal(String::new());
    #[cfg(feature = "ssr")]
    {
        set_inner.set(html_escape::encode_text(&code).into_owned());
    }
    #[cfg(not(feature = "ssr"))]
    {
        let result = crate::hljs::highlight(code, lang);
        Effect::new(move |_| {
            if let Some(r) = result.clone() { set_inner.set(r) }
        });
    }
    view! {
        <pre><code inner_html=inner></code></pre>
    }
}"#
    .to_string();
  let lang = "rust".to_string();
  provide_context(InnerEffect);

  view! {
      <h2>"Corrected example using signal + effect (again)."</h2>
      <CodeDemoWasmInner />
      <p>
          "
             Since the previous example didn't quite get everything working due to the component here providing
             different content between SSR and CSR, using client side signal and effect can opt-in the
             difference to overwrite the SSR rendering when hydration is complete.  This is pretty much the
             identical approach as example 5 as it is the idiomatic solution.  The improved version of the code
             rendering component from the previous example may look something like the following:
            "
      </p>
      <CodeInner code lang />
      <p>
          "
             With the use of effects, the expected final rendering after hydration and under CSR will be the
             highlighted version as expected.  As part of trial and error, the author previously tried to
             workaround this issue by using events via "<code>"web_sys"</code>
          " hack around signal, but again,
            using effects like so is a lot better for this particular library.
            "
      </p>
      <p>
          "
             Given the difference between CSR and SSR, the two different renderings are disambiguated via the
             use of "<code>"[cfg(feature = ...)]"</code>" for the available behavior.  If there is a
          corresponding API to provided highlighting markup under SSR, this feature gating would be managed
          at the library level and the component would simply call the "<code>"highlight"</code>
          " function
            directly, resulting in both SSR/CSR rendering being fully isomorphic even with JavaScript disabled
            on the client.
            "
      </p>
      <p>
          "
             To include the output of JavaScript code for SSR may be achieved in any of the following ways:
            "
      </p>
      <ul>
          <li>
              "
                    Run a JavaScript code in some JavaScript runtime such as Node.js, SpiderMonkey or Deno with
                    the input, and return the collected output.
                    "
          </li>
          <li>
              "
                    Use a JavaScript engine as above but more directly through some kind of Rust bindings through
                    packages such as "<code>"rusty_v8"</code>" or "<code>"mozjs"</code>".
              "
          </li>
          <li>
              "
                    Or go the full WASM route - compile the required JavaScript into WASM and use that through
                    Wasmtime on the server.
                    "
          </li>
      </ul>
      <p>
          "
             All of the above are very much outside the scope of this demo which is already showing the too
             many ways to include JavaScript into a Leptos project.
            "
      </p>
  }
}

#[component]
pub fn CodeDemoWasmInner() -> impl IntoView {
  let code = Resource::new(|| (), |_| fetch_code());
  let code_view = move || {
    Suspend::new(async move {
      code.await.map(|code| {
        view! { <CodeInner code=code lang="rust".to_string() /> }
      })
    })
  };
  view! {
      <p>
          "
          The following code examples are assigned via "<code>"inner_html"</code>
          " after processing through
          the relevant/available API call depending on SSR/CSR, without using any "
          <code>"web_sys"</code>"
          events or DOM manipulation outside of Leptos.
          "
      </p>
      <div id="code-demo">
          <table>
              <thead>
                  <tr>
                      <th>"Inline code block (part of this component)"</th>
                      <th>"Dynamic code block (loaded via server fn)"</th>
                  </tr>
              </thead>
              <tbody>
                  <tr>
                      <td>
                          <CodeInner code=CH03_05A.to_string() lang="rust".to_string() />
                      </td>
                      <td>
                          <Suspense fallback=move || {
                              view! { <p>"Loading code example..."</p> }
                          }>{code_view}</Suspense>
                      </td>
                  </tr>
              </tbody>
          </table>
      </div>
  }
}
