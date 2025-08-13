import * as React from "https://esm.sh/react@18.2.0";
import * as ReactDOMServer from "https://esm.sh/react-dom@18.2.0/server";

const App = () => {
  return React.createElement("div", null, "Hello from React SSR!");
};

const html = ReactDOMServer.renderToString(React.createElement(App));
console.log(html);
