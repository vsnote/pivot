import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyAppTheme, useAppStore } from "./store/useAppStore";

applyAppTheme(useAppStore.getState().theme);

void useAppStore.getState().initEventListeners();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
