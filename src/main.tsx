// [Overview: React rendering mount point. StrictMode is disabled to avoid double-mounting in development, which can interfere with the Tauri wgpu lifecycle.]
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    // We intentionally remove StrictMode here to avoid double-invocation
    // in dev mode, which can sometimes interfere with Tauri window initialization
    // and wgpu rendering lifecycle.
    <App />
);
