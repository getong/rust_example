use leptos::prelude::*;
use leptos_meta::*;

use crate::{api::fetch_code, consts::CH03_05A};

#[component]
pub fn CodeDemoSignalEffect() -> impl IntoView {
  // Full JS without the use of hydration event
  // this version will unset hljs if hljs was available to throw a wrench into
  // the works, but it should still just work.
  let render_call = r#"
if (window.hljs) {
    hljs.highlightAll();
    console.log('unloading hljs to try to force the need for addEventListener for next time');
    window['hljs'] = undefined;
} else {
    document.querySelector('#hljs-src')
        .addEventListener('load', (e) => {
            hljs.highlightAll();
            console.log('using hljs inside addEventListener; leaving hljs loaded');
        }, false);
};"#;
  let code = Resource::new(|| (), |_| fetch_code());
  let (script, set_script) = signal(None::<String>);
  let code_view = move || {
    Suspend::new(async move {
      Effect::new(move |_| {
        set_script.set(Some(render_call.to_string()));
      });
      view! {
          <pre>
              <code class="language-rust">{code.await}</code>
          </pre>
          {move || {
              script
                  .get()
                  .map(|script| {
                      view! { <Script>{script}</Script> }
                  })
          }}
      }
    })
  };
  view! {
      <Script id="hljs-src" async_="true" src="/highlight.min.js">
          ""
      </Script>
      <h2>
          "Using signal + effect to dynamically set "<code>"<Script>"</code>
          " tag as view is mounted"
      </h2>
      <p>
          "Explanation on what is being demonstrated follows after the following code example table."
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
                          <pre>
                              <code class="language-rust">{CH03_05A}</code>
                          </pre>
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
      <p>
          "
           To properly ensure the "<code>"<Script>"</code>
          " tag containing the initialization code for the
          target JavaScript usage is executed after the "<code>"Suspend"</code>
          "ed view is fully rendered
            and mounted onto the DOM, with the use of an effect that sets a signal to trigger the rendering
            inside the suspend will achieve exactly that.  That was a mouthful, so let's look at the code
            for that then:
            "
      </p>
      <div>
          <pre>
              <code class="language-rust">
                  r##"#[component]
                    fn CodeDemoSignalEffect() -> impl IntoView {
                    let render_call = r#"
                    if (window.hljs) {
                    hljs.highlightAll();
                    } else {
                    document.querySelector('#hljs-src')
                    .addEventListener('load', (e) => { hljs.highlightAll() }, false);
                    };"#;
                    let code = Resource::new(|| (), |_| fetch_code());
                    let (script, set_script) = signal(None::<String>);
                    let code_view = move || {
                    Suspend::new(async move {
                    Effect::new(move |_| {
                    set_script.set(Some(render_call.to_string()));
                    });
                    view! {
                    <pre><code class="language-rust">{code.await}</code></pre>
                    {
                    move || script.get().map(|script| {
                    view! { <Script>{script}</Script> }
                    })
                    }
                    }
                    })
                    };
                    view! {
                    <Script id="hljs-src" async_="true" src="/highlight.min.js">""</Script>
                    <Suspense fallback=move || view! { <p>"Loading code example..."</p> }>
                    {code_view}
                    </Suspense>
                    }
                    }"##
              </code>
          </pre>
      </div>
      <p>
          "
           The "<code>"Suspend"</code>" ensures the asynchronous "<code>"Resource"</code>
          " will be completed
            before the view is returned, which will be mounted onto the DOM, but the initial value of the
            signal "<code>"script"</code>" will be "<code>"None"</code>", so no "
          <code>"<Script>"</code>" tag
          will be rendered at that stage.  Only after the suspended view is mounted onto the DOM the "
          <code>"Effect"</code>" will run, which will call "<code>"set_script"</code>" with "
          <code>"Some"</code>"
          value which will finally populate the "<code>"<Script>"</code>
          " tag with the desired JavaScript to
            be executed, in this case invoke the code highlighting feature if available otherwise wait for it.
            "
      </p>
      <p>
          "
           If there are multiple "<code>"Suspense"</code>
          ", it will be a matter of adding the event to be
          dispatched to "<code>"set_script.set"</code>
          " so that it gets dispatched for the component, and
            then elsewhere above all those components a JavaScript list will tracking all the events will be
            waited on by "<code>"Promise.all"</code>
          ", where its completion will finally invoke the desired
            JavaScript function.
            "
      </p>
  }
}
