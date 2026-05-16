import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./theme/workbench-theme.css";
import "./workbench/WorkbenchShell.css";
import "./workbench/WorkbenchToolbar.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
