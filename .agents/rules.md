# CrystalCanvas Project Rules

## 1. 环境安装与工具链管理

**所有开发工具链和依赖环境应尽量安装在项目目录内**，避免污染系统全局环境。

### Rust 工具链
- 使用 `RUSTUP_HOME` 和 `CARGO_HOME` 环境变量将 Rust 工具链安装到项目本地：
  ```bash
  export RUSTUP_HOME="$PWD/.rustup"
  export CARGO_HOME="$PWD/.cargo"
  ```
- 对应的 `.rustup/` 和 `.cargo/` 目录已在 `.gitignore` 中排除。

### Node.js
- 推荐使用 `volta` 或 `fnm` 管理 Node 版本，配置项目级固定版本。
- 若需本地安装，使用 `.node/` 目录（已在 `.gitignore` 中排除）。

### Python (辅助脚本)
- 若项目中有 Python 脚本，使用项目内 `.venv/` 虚拟环境。

---

## 2. Git 版本控制规则

### 不纳入版本控制的内容
以下文件仅存在于本地开发环境，**不得上传到 GitHub**：

| 类别 | 路径 | 说明 |
|---|---|---|
| 路线图 | `roadmap.md` | 内部规划文档 |
| 技术设计文档 | `docs/` | TDD、架构设计等 |
| 环境配置 | `.env`, `.env.*` | API 密钥、本地配置 |
| 本地工具链 | `.rustup/`, `.cargo/`, `.node/` | 项目内安装的工具链 |
| 构建产物 | `target/`, `build/`, `dist/`, `node_modules/` | 编译输出 |

### 纳入版本控制的内容
- 所有源代码（Rust, C++, TypeScript, WGSL shaders）
- `README.md`
- `.gitignore`
- `Cargo.toml`, `package.json`, `tauri.conf.json`
- `CMakeLists.txt`
- License 文件

---

## 3. 开发平台优先级

| 优先级 | 平台 | 说明 |
|---|---|---|
| **P0** | macOS Intel | 当前主力开发机，Metal 2.0 后端 |
| **P1** | macOS Apple Silicon | 未来主力，统一内存优势 |
| **P2** | Ubuntu 22.04+ | Vulkan 后端 |
| **P3** | Windows | 延后，社区驱动 |

---

## 4. 代码风格与约定

### 语言规则
- **所有代码和注释必须使用英文**，包括变量名、函数名、注释、文档字符串、commit message。
- 仅 `.agents/` 下的项目规则文档和对话输出可以使用中文。

### Rust
- 遵循 `cargo fmt` 和 `cargo clippy` 规范。
- FFI 边界使用 `cxx` bridge，C++ 异常不得穿越 FFI 边界。
- **License 声明**：后续初始化 `src-tauri/Cargo.toml` 时，必须在 `[package]` 中显式声明：
  ```toml
  license = "MIT OR Apache-2.0"
  ```
  以确保发布和分发时自动化工具能正确识别双协议授权。


### C++
- 对外接口使用 Thin C Wrapper（`extern "C"` 风格函数签名）。
- Gemmi/Eigen 的 `#include` 仅在 `.cpp` 实现文件中，不泄漏到头文件。

### TypeScript / React
- 使用 TailwindCSS 进行样式管理。
- 通过 Tauri `invoke()` 与 Rust 后端通信，不直接操作任何物理状态。

### WGSL Shaders
- 统一使用 WGSL 作为唯一着色器源码语言。
- 通过 naga 自动编译为 Metal MSL / SPIR-V / HLSL。
- 不使用任何平台特定的 shader 扩展。

---

## 5. 构建规则

- **一条命令构建**：`cargo build` 必须能完成 Rust + C++ 的完整编译。
- C++ 依赖（Spglib, Gemmi）作为 git submodule 引入，静态编译（Vendored）。
- macOS 开发仅需 Xcode Command Line Tools，零额外依赖。

---

## 6. 代码文件首行注释规则

**所有新生成的源代码文件必须在首行添加功能概述注释**，便于 LLM 快速识别文件用途，无需全量阅读代码。

### 格式规范

| 语言 | 注释格式 |
|---|---|
| Rust | `//! [功能概述：一句话描述文件职责]` |
| C++ (.hpp/.cpp) | `// [功能概述：一句话描述文件职责]` |
| TypeScript | `// [功能概述：一句话描述文件职责]` |
| WGSL | `// [功能概述：一句话描述文件职责]` |

### 示例
```rust
//! CIF/PDB 文件解析的 Rust ↔ C++ FFI 桥接层，使用 cxx 生成安全绑定
```
```cpp
// Gemmi CIF 解析的 Thin C Wrapper，将 SmallStructure 转换为 FFI 安全的 POD 数据
```

### 规则
1. **首行必须是功能概述**，不超过一行，描述文件"做什么"而非"如何做"
2. 版权声明（如有）放在功能概述**之后**
3. 功能概述应当让 LLM 仅凭这一行就能判断是否需要深入阅读该文件

---

## 7. 第三方库 API 参考

**第三方库的调用规则和常用 API 汇总在** `.agents/third_party_api.md` 中。

### 目的
- LLM 在首次扫描第三方库源码后，将使用模式写入此文档
- 后续调用时，LLM 直接查阅此文档即可，无需重新扫描第三方库源码
- 大幅减少 LLM 上下文长度，降低项目崩溃风险

### 规则
1. 每个第三方库占一个独立章节
2. 记录：头文件路径、关键类/函数签名、典型使用示例代码片段
3. 每当引入新的第三方库 API 调用时，**必须同步更新此文档**
