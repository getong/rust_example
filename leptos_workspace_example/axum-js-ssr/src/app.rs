use leptos::prelude::*;
use leptos_meta::{MetaTags, *};
use leptos_router::{
  SsrMode,
  components::{A, FlatRoutes, Route, Router},
  path,
};

use crate::{
  api::fetch_code,
  component::{
    custom_event::CustomEvent, naive_alt::NaiveEvent, native::Naive,
    signal_effect_script::CodeDemoSignalEffect, wasm_bindgen_direct::WasmBindgenDirect,
    wasm_bindgen_direct_fixed::WasmBindgenDirectFixed, wasm_bindgen_effect::WasmBindgenEffect,
    wasm_bindgen_js_hook_ready_event::WasmBindgenJSHookReadyEvent,
    wasm_bindgen_naive::WasmBindgenNaive,
  },
  consts::CH03_05A,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
  view! {
      <!DOCTYPE html>
      <html lang="en">
          <head>
              <meta charset="utf-8" />
              <meta name="viewport" content="width=device-width, initial-scale=1" />
              <AutoReload options=options.clone() />
              <HydrationScripts options />
              <MetaTags />
          </head>
          <body>
              <App />
          </body>
      </html>
  }
}

#[component]
pub fn App() -> impl IntoView {
  // Provides context that manages stylesheets, titles, meta tags, etc.
  provide_meta_context();
  let fallback = || view! { "Page not found." }.into_view();

  view! {
      <Stylesheet id="leptos" href="/pkg/axum_js_ssr.css" />
      <Title text="Leptos JavaScript Integration Demo with SSR in Axum" />
      <Meta name="color-scheme" content="dark light" />
      <Router>
          <nav>
              <A attr:class="section" href="/">
                  "Introduction (home)"
              </A>
              <A attr:class="example" href="/naive">
                  "Naive "
                  <code>"<script>"</code>
                  <small>"truly naive to start off"</small>
              </A>
              <A attr:class="example" href="/naive-alt">
                  "Leptos "
                  <code>"<Script>"</code>
                  <small>"naively using load event"</small>
              </A>
              <A attr:class="example" href="/naive-hook">
                  "Leptos "
                  <code>"<Script>"</code>
                  <small>"... correcting placement"</small>
              </A>
              <A attr:class="example" href="/naive-fallback">
                  "Leptos "
                  <code>"<Script>"</code>
                  <small>"... with fallback"</small>
              </A>
              <A attr:class="example" href="/signal-effect-script">
                  "Leptos Signal + Effect"
                  <small>"an idiomatic Leptos solution"</small>
              </A>
              <A attr:class="subexample section" href="/custom-event">
                  "Hydrated Event"
                  <small>"using "<code>"js_sys"</code>"/"<code>"web_sys"</code></small>
              </A>
              <A attr:class="example" href="/wasm-bindgen-naive">
                  "Using "
                  <code>"wasm-bindgen"</code>
                  <small>"naively to start with"</small>
              </A>
              <A attr:class="example" href="/wasm-bindgen-event">
                  "Using "
                  <code>"wasm-bindgen"</code>
                  <small>"overcomplication with events"</small>
              </A>
              <A attr:class="example" href="/wasm-bindgen-effect">
                  "Using "
                  <code>"wasm-bindgen"</code>
                  <small>"lazily delay DOM manipulation"</small>
              </A>
              <A attr:class="example" href="/wasm-bindgen-direct">
                  "Using "
                  <code>"wasm-bindgen"</code>
                  <small>"without DOM manipulation"</small>
              </A>
              <A attr:class="example section" href="/wasm-bindgen-direct-fixed">
                  "Using "
                  <code>"wasm-bindgen"</code>
                  <small>"corrected with signal + effect"</small>
              </A>
              <a id="reset" href="/" target="_self">
                  "Restart/Rehydrate"
                  <small>"to make things work again"</small>
              </a>
          </nav>
          <main>
              <div id="notice">
                  "The WASM application has panicked during hydration. " <a href="/" target="_self">
                      "Restart the application by going home"
                  </a>"."
              </div>
              <article>
                  <h1>"Leptos JavaScript Integration Demo with SSR in Axum"</h1>
                  <FlatRoutes fallback>
                      <Route path=path!("") view=HomePage />
                      <Route path=path!("naive") view=Naive ssr=SsrMode::Async />
                      <Route
                          path=path!("naive-alt")
                          view=|| view! { <NaiveEvent /> }
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("naive-hook")
                          view=|| view! { <NaiveEvent hook=true /> }
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("naive-fallback")
                          view=|| view! { <NaiveEvent hook=true fallback=true /> }
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("signal-effect-script")
                          view=CodeDemoSignalEffect
                          ssr=SsrMode::Async
                      />
                      <Route path=path!("custom-event") view=CustomEvent ssr=SsrMode::Async />
                      <Route
                          path=path!("wasm-bindgen-naive")
                          view=WasmBindgenNaive
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("wasm-bindgen-event")
                          view=WasmBindgenJSHookReadyEvent
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("wasm-bindgen-effect")
                          view=WasmBindgenEffect
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("wasm-bindgen-direct")
                          view=WasmBindgenDirect
                          ssr=SsrMode::Async
                      />
                      <Route
                          path=path!("wasm-bindgen-direct-fixed")
                          view=WasmBindgenDirectFixed
                          ssr=SsrMode::Async
                      />
                  </FlatRoutes>
              </article>
          </main>
      </Router>
  }
}

#[component]
fn HomePage() -> impl IntoView {
  view! {
      <p>
          "
             This example application demonstrates a number of ways that JavaScript may be included and used
             with Leptos naively, describing and showing the shortcomings and failures associated with each of
             them for both SSR (Server-Side Rendering) and CSR (Client-Side Rendering) with hydration, before
             leading up to the idiomatic solutions where they work as expected.
            "
      </p>
      <p>
          "
           For the demonstrations, "<a href="https://github.com/highlightjs/highlight.js">
              <code>"highlight.js"</code>
          </a>" will be invoked from within this Leptos application by the examples
          linked on the side bar.  Since the library to be integrated is a JavaScript library, it must be
          enabled to fully appreciate this demo, and having the browser's developer tools/console opened is
          recommended as the logs will indicate the effects and issues as they happen.
          "
      </p>
      <p>
          "
           Examples 1 to 5 are primarily JavaScript based, where the integration code is included as "
          <code>"<script>"</code>
          " tags, with example 5 (final example of the group) being the idiomatic solution
            that runs without errors or panic during hydration, plus an additional example 5.1 showing how to
            get hydration to dispatch an event for JavaScript libraries should that be required.  Examples 6
            to 10 uses "<code>"wasm-bindgen"</code>
          " to call out to the JavaScript library from Rust, starting
            off with naive examples that mimics JavaScript conventions, again with the final example of the
            group (example 10) being the fully working version that embraces the use of Rust.
            "
      </p>
  }
}

#[derive(Clone, Debug)]
pub struct CodeDemoHook {
  pub js_hook: String,
}

#[component]
pub fn CodeDemo() -> impl IntoView {
  let code = Resource::new(|| (), |_| fetch_code());
  let code_view = move || {
    Suspend::new(async move {
      let hook = use_context::<CodeDemoHook>().map(|h| {
        leptos::logging::log!("use context suspend JS");
        view! { <Script>{h.js_hook}</Script> }
      });
      view! {
          <pre>
              <code class="language-rust">{code.await}</code>
          </pre>
          {hook}
      }
    })
  };
  view! {
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
  }
}

pub enum WasmDemo {
  Naive,
  ReadyEvent,
  RequestAnimationFrame,
}

#[component]
pub fn CodeDemoWasm(mode: WasmDemo) -> impl IntoView {
  let code = Resource::new(|| (), |_| fetch_code());
  let suspense_choice = match mode {
    WasmDemo::Naive => view! {
        <Suspense fallback=move || {
            view! { <p>"Loading code example..."</p> }
        }>
            {move || Suspend::new(async move {
                view! {
                    <pre>
                        <code class="language-rust">{code.await}</code>
                    </pre>
                    {#[cfg(feature = "hydrate")]
                    {
                        use crate::hljs::highlight_all;
                        leptos::logging::log!("calling highlight_all");
                        highlight_all();
                    }}
                }
            })}
        </Suspense>
    }
    .into_any(),
    WasmDemo::ReadyEvent => view! {
        <Suspense fallback=move || {
            view! { <p>"Loading code example..."</p> }
        }>
            {move || Suspend::new(async move {
                view! {
                    <pre>
                        <code class="language-rust">{code.await}</code>
                    </pre>
                    {#[cfg(feature = "hydrate")]
                    {
                        use crate::hljs;
                        use wasm_bindgen::{closure::Closure, JsCast};
                        let document = document();
                        let hydrate_listener = Closure::<
                            dyn Fn(_),
                        >::new(move |_: web_sys::Event| {
                                leptos::logging::log!("wasm hydration_listener highlighting");
                                hljs::highlight_all();
                            })
                            .into_js_value();
                        document
                            .add_event_listener_with_callback(
                                crate::consts::LEPTOS_HYDRATED,
                                hydrate_listener.as_ref().unchecked_ref(),
                            )
                            .expect("failed to add event listener to document");
                        let csr_listener = Closure::<
                            dyn FnMut(_),
                        >::new(move |_: web_sys::Event| {
                                leptos::logging::log!("wasm csr_listener highlighting");
                                hljs::highlight_all();
                            })
                            .into_js_value();
                        let options = web_sys::AddEventListenerOptions::new();
                        options.set_once(true);
                        document
                            .add_event_listener_with_callback_and_add_event_listener_options(
                                "hljs_hook",
                                csr_listener.as_ref().unchecked_ref(),
                                &options,
                            )
                            .expect("failed to add event listener to document");
                        leptos::logging::log!("wasm csr_listener listener added");
                        request_animation_frame(move || {
                            let event = web_sys::Event::new("hljs_hook")
                                .expect("error creating hljs_hook event");
                            document
                                .dispatch_event(&event)
                                .expect("error dispatching hydrated event");
                        });
                    }}
                }
            })}
        </Suspense>
    }
    .into_any(),
    WasmDemo::RequestAnimationFrame => view! {
        <Suspense fallback=move || {
            view! { <p>"Loading code example..."</p> }
        }>
        {#[cfg(feature = "hydrate")]
         move || Suspend::new(async move {
             Effect::new(move |_| {
                 request_animation_frame(move || {
                     leptos::logging::log!(
                         "request_animation_frame invoking hljs::highlight_all"
                     );
                     crate::hljs::highlight_all();
                 });
             });
             // under SSR this is an noop, but it wouldn't be called under there anyway because
             // it isn't the isomorphic version, i.e. Effect::new_isomorphic(...).
             view! {
                 <pre>
                     <code class="language-rust">{code.await}</code>
                     </pre>
             }
         })
        }

        </Suspense>
    }
    .into_any(),
  };
  view! {
      <p>
          "
           The syntax highlighting shown in the table below is done by invoking "
          <code>"hljs.highlightAll()"</code>" via the binding generated using "
          <code>"wasm-bindgen"</code>" - thus the ES version of " <code>"highlight.js"</code>
          " is loaded by the output bundle generated by Leptos under this set of
            demonstrations. However, things may still not work as expected, with the explanation on what is
            being demonstrated follows after the following code example table.
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
                          <pre>
                              <code class="language-rust">{CH03_05A}</code>
                          </pre>
                      </td>
                      <td>{suspense_choice}</td>
                  </tr>
              </tbody>
          </table>
      </div>
  }
}

#[derive(Clone)]
pub struct InnerEffect;

#[component]
pub fn CodeInner(code: String, lang: String) -> impl IntoView {
  // lang is currently unused for SSR, so just drop it now to use it to avoid warning.
  #[cfg(feature = "ssr")]
  drop(lang);
  if use_context::<InnerEffect>().is_none() {
    #[cfg(feature = "ssr")]
    let inner = Some(html_escape::encode_text(&code).into_owned());
    #[cfg(feature = "hydrate")]
    let inner = {
      let inner = crate::hljs::highlight(code, lang);
      leptos::logging::log!("about to populate inner_html with: {inner:?}");
      inner
    };
    view! {
        <pre>
            <code inner_html=inner></code>
        </pre>
    }
    .into_any()
  } else {
    let (inner, set_inner) = signal(String::new());
    #[cfg(feature = "ssr")]
    {
      set_inner.set(html_escape::encode_text(&code).into_owned());
    };
    #[cfg(feature = "hydrate")]
    {
      leptos::logging::log!("calling out to hljs::highlight");
      let result = crate::hljs::highlight(code, lang);
      Effect::new(move |_| {
        leptos::logging::log!("setting the result of hljs::highlight inside an effect");
        if let Some(r) = result.clone() {
          set_inner.set(r)
        }
      });
    };
    view! {
        <pre>
            <code inner_html=inner></code>
        </pre>
    }
    .into_any()
  }
}
