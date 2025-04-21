import "fast-text-encoding"; // Mandatory for React18
import { renderToString } from "react-dom/server";
import App from "./App";

export const Server = () => {
  const app = renderToString(<App />);
  return `<!doctype html>
  <html>
  <head>
  <title>React + Rust = ❤️</title>
  <script type="module" crossorigin src="/index.js"></script>
  </head>
  <body>
  <div id="root">${app}</div>
  </body>
  </html>`;
};