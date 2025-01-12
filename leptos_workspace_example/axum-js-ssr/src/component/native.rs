use leptos::prelude::*;

use crate::app::CodeDemo;

#[component]
pub fn Naive() -> impl IntoView {
  let loader = r#"<script src="/highlight.min.js"></script>
<script>hljs.highlightAll();</script>"#;
  view! {
      <h2>"Showing what happens when script inclusion is done naively"</h2>
      <CodeDemo />
      <p>
          "
           This page demonstrates what happens (or doesn't happen) when it is assumed that the "
          <code>"highlight.js"</code>
          " library can just be included from some CDN (well, hosted locally for this
            example) as per their instructions for basic usage in the browser, specifically:
            "
      </p>
      <div>
          <pre>
              <code class="language-html">{loader}</code>
          </pre>
      </div>
      <p>
          "
             The following actions should be taken in order to fully experience the things that do not work as
             expected:
            "
      </p>
      <ol>
          <li>
              "
                You may find that during the initial load of this page when first navigating to here from
                \"Introduction\" (do navigate there, reload to reinitiate this application to properly
                replicate the behavior, or simply use the Restart link at the bottom), none of the code
                examples below are highlighted.
                "
          </li>
          <li>
              "
                Go back and then forward again using the browser's navigation system the inline code block
                will become highlighted.  The cause is due to "<code>"highlight.js"</code>
              " being loaded in a
              standard "<code>"<script>"</code>
              " tag that is part of this component and initially it wasn't
              loaded before the call to "<code>"hljs.highlightAll();"</code>
              " was made. Later, when the
                component gets re-rendered the second time, the code is finally available to ensure one of
                them works (while also reloading the script, which probably isn't desirable for this use
                case).
                "
          </li>
          <li>
              "
              If you have the browser reload this page, you will find that " <strong>"both"</strong>
              " code
              examples now appear to highlight correctly, yay! However you will also find that the browser's
              back button appears to do nothing at all (even though the address bar may have changed), and
              that most of the links on the side-bar are non-functional.  A message should have popped up at
              the top indicating that the application has panicked.
              "
              <details>
                  "
                  "<summary>"Details about the cause of the crash:"</summary>
                  <p>
                      "
                         The cause here is because the hydration system found a node where text was expected, a
                         simple violation of the application's invariant.  Specifically, the code block
                         originally contained plain text, but with highlighting that got changed to some HTML
                         markup "<em>"before"</em>
                      " hydration happened, completely ouside of expectations.
                        Generally speaking, a panic is the worst kind of error, as it is a hard crash which
                        stops the application from working, and in this case the reactive system is in a
                        completely non-functional state.
                        "
                  </p>
                  <p>
                      "
                         Fortunately for this application, some internal links within this application have
                         been specifically excluded from the reactive system (specifically the restart links,
                         so they remain usable as they are just standard links which include the bottommost one
                         of the side bar and the one that should become visible as a notification as the panic
                         happened at the top - both may be used to navigate non-reactively back to the
                         homepage.
                        "
                  </p>
                  <p>
                      "
                         Navigating back after using the non-reactive links will also restart the application,
                         so using that immediately after to return to this page will once again trigger the
                         same condition that will result the hydration to panic.  If you wish to maintain the
                         push state within the history, simply use the browser navigation to navigate through
                         those pushed addresses and find one that may be reloaded without causing the crash,
                         and then go the opposite direction the same number of steps to get back to here.
                        "
                  </p>"
                  "
              </details>"
              "
          </li>
          <li>
              "
                In the working CSR state, if you continue to use the browser's navigation system to go back to
                home and forward back to this page, you will find that the the browser's console log is
                spammed with the different delays added to the loading of the standard highlight.js file.  The
                cause is because the script is unloaded/reloaded every time its "
              <code>"<script>"</code>" tag
              is re-created by this component.  This may or may not be a desirable behavior, so where
              exactly these tags are situated will matter - if the goal is to load the script once, the tag
              should be provided above the Router.
              "
          </li>
          <li>
              "
                Simply use the restart links to get back home and move onto the next example - or come back
                here, if you wish - while all the examples can be used out of order, the intended broken
                behaviors being demonstrated are best experienced by going home using the reactive link at the
                top, and go back to the target example.  Going between different examples demonstrating the
                subtly broken behavior(s) in arbitrary order can and will amplify into further unexpected and
                potentially hard to reproduce behaviors.  What they are and why they happen are left as
                exercise for the users and readers of this demo application.
                "
          </li>
      </ol>
      <script src="/highlight.min.js"></script>
      <script>"hljs.highlightAll();"</script>
  }
}
