# Contributing to CrystalCanvas

Thank you for your interest in contributing to CrystalCanvas! We welcome contributions from researchers, software engineers, and crystal structure modeling enthusiasts.

---

## 🚀 Getting Started

### 1. Set Up Your Development Environment
CrystalCanvas is a cross-language project (Rust + C++ + TypeScript). We recommend installing toolchains locally within the project directory to avoid system conflicts.

- **macOS (Primary)**: Install Xcode Command Line Tools: `xcode-select --install`.
- **Linux (Ubuntu)**: Install `build-essential`, `cmake`, `libgtk-3-dev`, and `libwebkit2gtk-4.1-dev`.
- **Rust**: Source the `dev_env.sh` script (`source dev_env.sh`) to initialize project-local `.rustup` and `.cargo` directories, then install the stable toolchain: `rustup toolchain install stable`.
- **Node.js / package manager**: We strictly use **`pnpm`** to manage dependencies and avoid phantom dependency issues. Do not use `npm` or `yarn`.

### 2. Fork and Clone
1. Fork the repository on GitHub.
2. Clone your fork locally:
   ```bash
   git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
   cd CrystalCanvas
   ```

### 3. Build & Run the Project
We use a unified build system powered by Tauri. Running the development server will automatically handle both the frontend bundle and the Rust/C++ compilation.
```bash
# 1. Install frontend dependencies
pnpm install

# 2. Start the application (compiles Rust + C++ automatically)
source dev_env.sh
pnpm run dev
```

---

## 🛠️ Development Workflow

1. **Create a Branch**: Always work on a new branch for your feature or fix.
   ```bash
   git checkout -b feature/your-feature-name
   ```
2. **Make Changes**: Follow the [Coding Guidelines](#-coding-guidelines).
3. **Commit**: Use descriptive commit messages (e.g., `feat: add slab cleaving for hexagonal tiles`).
4. **Push & PR**: Push to your fork and open a Pull Request against the `main` branch.

---

## 📏 Coding Guidelines

### Rust (Backend & Orchestration)
- Use `cargo fmt` to format code.
- Run `cargo clippy` to check for common mistakes.
- All core state must reside in the Rust layer (Single Source of Truth).
- FFI boundaries must use the `cxx` bridge.

### C++ (Physics Kernel)
- Keep the public API minimal using "Thin C Wrappers".
- Use `Eigen` for linear algebra and `Spglib` for symmetry.
- Ensure all C++ exceptions are caught within the wrapper and converted to Rust `Result` types.

### Web (React + TypeScript)
- **UI Frameworks Banned**: Build all components from scratch using **pure TailwindCSS** classes. Do not use UI libraries like Headless UI, DaisyUI, or Radix UI.
- **Strict IPC Typing**: Any data crossing the Rust ↔ TypeScript boundary (like `CrystalState` or `CrystalCommand`) must have a strict 1:1 mapped TypeScript interface located in `src/types/`. Avoid `any`.
- Avoid holding physical state in the UI; use the Command Bus to interact with the backend.

### Shaders (WGSL)
- Write shaders in WGSL.
- Do not use platform-specific extensions to ensure compatibility across Metal, Vulkan, and DX12.

---

## 🧪 Testing

- **Rust**: Run `cargo test`.
- **C++**: We use unit tests within the C++ kernel directory.
- **Visual**: For rendering changes, verify on at least **macOS Intel** (our baseline) or **Ubuntu**.

---

## ⚖️ License

By contributing to CrystalCanvas, you agree that your contributions will be licensed under the project's **dual MIT and Apache-2.0 license**.

---

## 💬 Communication

If you have questions or want to discuss a large feature before starting work, please open an **Issue** or join our community discussions.

*Happy Modeling!*
