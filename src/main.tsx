// [Overview: React rendering mount point. StrictMode is disabled to avoid double-mounting in development, which can interfere with the Tauri wgpu lifecycle.]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

window.onerror = function (msg, url, lineNo, columnNo, error) {
    document.body.innerHTML = `
    <div style="background: red; color: white; padding: 20px; font-family: monospace; z-index: 99999; position: absolute; top:0; left:0; right:0; bottom:0;">
      <h1>GLOBAL ERROR CAUGHT</h1>
      <p>${msg}</p>
      <p>${url}:${lineNo}:${columnNo}</p>
      <pre>${error?.stack}</pre>
    </div>
  `;
    return false;
};

class ErrorBoundary extends React.Component<any, any> {
    constructor(props: any) { super(props); this.state = { hasError: false, error: null }; }
    static getDerivedStateFromError(error: any) { return { hasError: true, error }; }
    componentDidCatch(error: any, errorInfo: any) { console.error("Caught component error:", error, errorInfo); }
    render() {
        if (this.state.hasError) {
            return <div style={{ background: 'darkred', color: 'white', padding: 20, zIndex: 99999, position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 }}>
                <h1>React Component Error Caught</h1>
                <pre>{String(this.state.error.stack || this.state.error)}</pre>
            </div>;
        }
        return this.props.children;
    }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <ErrorBoundary>
        <App />
    </ErrorBoundary>
);
