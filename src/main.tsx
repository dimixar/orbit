import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// Locally-bundled code fonts (not on Google Fonts) — work offline.
import "@fontsource/commit-mono/400.css";
import "@fontsource/commit-mono/500.css";
import "@fontsource/commit-mono/600.css";
import "@fontsource/iosevka/400.css";
import "@fontsource/iosevka/500.css";
import "@fontsource/iosevka/600.css";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
