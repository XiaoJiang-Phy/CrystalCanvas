// [功能概述：React 渲染挂载点，禁用 StrictMode 以避免开发环境下组件的双重挂载可能导致 Tauri wgpu 的生命周期问题]
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
