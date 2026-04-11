# Contributing to CrystalCanvas

Thank you for your interest in contributing to CrystalCanvas! We welcome contributions from researchers, software engineers, and crystal structure modeling enthusiasts.

---

## Getting Started

### 1. Set Up Your Development Environment

CrystalCanvas is a cross-language project (Rust + C++ + TypeScript). All toolchains are isolated within the project directory via a **Zero-Global-Pollution** strategy.

- **macOS (Primary)**: Install Xcode Command Line Tools: `xcode-select --install`.
- **Linux (Ubuntu)**: Install `build-essential`, `cmake`, `libgtk-3-dev`, and `libwebkit2gtk-4.1-dev`.
- **Rust**: Source the `dev_env.sh` script (`source dev_env.sh`) to initialize project-local `.rustup` and `.cargo` directories, then install the stable toolchain: `rustup toolchain install stable`.
- **Node.js / package manager**: We strictly use **`pnpm`** to manage dependencies. Do not use `npm` or `yarn`.

### 2. Fork and Clone

1. Fork the repository on GitHub.
2. Clone your fork locally:
   ```bash
   git clone https://github.com/XiaoJiang-Phy/CrystalCanvas.git
   cd CrystalCanvas
   ```

### 3. Build & Run

We use a unified build system powered by Tauri. The dev server handles frontend bundling and Rust/C++ compilation automatically.

```bash
# 1. Initialize the local Rust toolchain
source dev_env.sh

# 2. Install frontend dependencies
pnpm install

# 3. Start the application (compiles Rust + C++ automatically)
pnpm run tauri dev
```

> **Note**: The C++ kernel (Spglib, Gemmi, Eigen) is compiled automatically via `build.rs` using `cxx-build`. No manual CMake interaction is required.

---

## Development Workflow

1. **Create a Branch**: Always work on a new branch for your feature or fix.
   ```bash
   git checkout -b feature/your-feature-name
   ```
2. **Make Changes**: Follow the [Coding Guidelines](#coding-guidelines) below.
3. **Commit**: Use [Conventional Commits](https://www.conventionalcommits.org/) format:
   - `feat: add distance/angle measurement tool`
   - `fix: correct CHGCAR coordinate alignment`
   - `refactor: extract StateTransaction helper from commands.rs`
4. **Push & PR**: Push to your fork and open a Pull Request against the `main` branch.

---

## Coding Guidelines

### Naming Convention

| Category | Convention | Examples |
|---|---|---|
| Variables / Functions | `snake_case` | `sigma_k`, `calculate_self_energy` |
| Types / Classes | `PascalCase` | `CrystalState`, `BondInstance` |
| Constants | `UPPER_CASE` | `MAX_ATOMS`, `PI` |
| Template Arguments | `PascalCase` | `ScalarType`, `DevicePolicy` |

**Physics symbol fidelity**: Preserve mathematical case sensitivity — `delta_k` ≠ `Delta_K`.

### Rust (Backend & Orchestration)

- Use `cargo fmt` and `cargo clippy` before committing.
- All core crystal state must reside in the Rust layer (Single Source of Truth, SSoT).
- **Dual-precision discipline**: Use `f64` for physics calculations (fractional coordinates), `f32` for GPU rendering (Cartesian positions).
- **ColMajor enforcement**: All lattice matrices follow Fortran column-major order. Never transpose implicitly.
- **Lock ordering**: When acquiring multiple `Mutex` locks, always follow `crystal_state → settings → renderer` order to prevent deadlocks.
- FFI boundaries must use the `cxx` bridge. Do not use raw `extern "C"` unless `cxx` is insufficient for a specific interface.
- Use the `with_state_update` transaction helper (see [TDD §6.2.1](doc/TDD_CrystalCanvas_v1.md)) for any command that mutates `CrystalState`, instead of manual lock-mutate-rebuild-emit boilerplate.

### C++ (Physics Kernel)

- Keep the public API minimal using "Thin C Wrappers".
- Use `Eigen` for linear algebra and `Spglib` for symmetry analysis.
- All C++ exceptions must be caught within the wrapper and converted to Rust `Result` types. Exceptions must **never** cross the FFI boundary.
- Do not use `using namespace std;` or `using namespace Eigen;` — all external library calls must be explicitly qualified.
- Comment discipline: only `///` Doxygen docs, physics formula references (e.g., `// Eq.(3.12) [Mahan00]`), and non-obvious technical rationale. No "Step 1 / Step 2" narration comments.

### Web (React + TypeScript)

- **UI Frameworks Banned**: Build all components from scratch using **pure TailwindCSS** classes. Do not use UI libraries like Headless UI, DaisyUI, or Radix UI.
- **Strict IPC Typing**: Any data crossing the Rust ↔ TypeScript boundary (e.g., `CrystalState`, `CrystalCommand`) must have a strict 1:1 mapped TypeScript interface in `src/types/`. Avoid `any`.
- Do not hold physical state in the UI. Use the Command Bus to interact with the backend.
- When splitting large components, use `React.lazy()` for panel-level code splitting.

### Shaders (WGSL)

- Write all shaders in WGSL (the sole source language, cross-compiled by `naga`).
- Do not use platform-specific extensions to ensure compatibility across Metal, Vulkan, and DX12.
- Only use features guaranteed by `wgpu::DownlevelFlags::default()`.

---

## Architecture Overview

```
L4: React + TypeScript + TailwindCSS    (Presentation)
L3: Rust / Tauri 2.0                    (Application Logic / SSoT)
L2: Rust / wgpu                         (Rendering Engine)
L1: C++ (Spglib / Gemmi / Eigen)        (Physics Kernel)
```

Key architectural rules:

- **L4** is a pure presentation layer — no physics state caching.
- **L3** owns all crystal state and orchestrates L1 ↔ L2 communication.
- **L2** holds only GPU-side rendering buffer mirrors — no physical logic.
- **L1** is a stateless computation engine: input → output, no state caching.

For detailed technical design, see [TDD](doc/TDD_CrystalCanvas_v1.md) and [Roadmap](doc/roadmap.md).

---

## Testing

- **Rust**: `cargo test` in `src-tauri/`. All tests must pass before PR submission.
- **C++**: Unit tests within `cpp/` via CMake CTest.
- **Visual verification**: For rendering changes, verify on at least **macOS Intel** (our development baseline). Screenshots of before/after are encouraged in the PR description.
- **Performance thresholds**: Do not modify timing thresholds or tolerance constants to make a failing test pass. Fix the underlying logic. See [TDD §5.6](doc/TDD_CrystalCanvas_v1.md) for immutable test hard-lines.

---

## Security

- **API Keys**: LLM API keys are stored in the OS Keychain via Tauri's secure storage. Never log, hardcode, or commit API keys.
- **LLM output is untrusted**: All AI-generated commands pass through Schema Validation → Physics Sandbox → Undo Snapshot before execution.

---

## License

By contributing to CrystalCanvas, you agree that your contributions will be licensed under the project's **dual MIT and Apache-2.0 license**.

---

## Communication

If you have questions or want to discuss a large feature before starting work, please open an **Issue** or join our community discussions.
