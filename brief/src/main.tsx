import React from "react";
import ReactDOM from "react-dom/client";
import i18n from "./i18n/index";
import App from "./App";

if (import.meta.env.DEV) {
  (window as unknown as { i18n: typeof i18n }).i18n = i18n;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
