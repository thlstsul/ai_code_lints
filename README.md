# ai_code_lints

针对 AI 生成代码常见问题的 [dylint](https://github.com/trailofbits/dylint) lint 集合。

## Lint 一览

| Lint | 检测内容 |
|------|----------|
| `TEST_ONLY_DEAD_CODE` | 仅被测试代码或 `pub use` 再导出引用的 `pub` 项（rustc / clippy 的死代码分析检不出来） |
| `SCATTERED_USE_STATEMENT` | `use` 语句未集中到模块头部（分散在模块各处或函数体内） |
| `FULLY_QUALIFIED_PATH_WITHOUT_USE` | 使用全限定路径而不通过 `use` 导入 |

## 安装与使用

前置条件：`rustup` 与 [dylint](https://github.com/trailofbits/dylint)（`cargo install cargo-dylint dylint-link`）。

本库依赖 `rustc_private` 内部 API，需要 nightly 工具链（当前钉定为 `nightly-2026-09-03`，见 `rust-toolchain.toml`，需安装 `rustc-dev` 组件）。

```bash
# 构建 lint 库
cargo build

# 对目标工程运行所有 lint（DYLINT_LIBRARY_PATH 指向本库 target/debug）
DYLINT_LIBRARY_PATH=/path/to/ai_code_lints/target/debug cargo dylint --all
```

单个 lint 可通过 `--allow`/`--warn`/`--deny`/`--forbid` 调整级别，如 `cargo dylint test_only_dead_code -- --deny ...`；也可在代码里 `#[allow(test_only_dead_code)]` 局部豁免。

## 配置

两个 lint 支持通过目标工程**工作区根目录**的 `dylint.toml` 配置：

```toml
# TEST_ONLY_DEAD_CODE：跨 crate（同 workspace 其他成员）的引用不可见，
# 完全未被引用的项可能误报。设为 true 时只报告确实被测试代码/再导出
# 引用的项，完全未引用的项交由 rustc 死代码分析处理。
[test_only_dead_code]
ignore_unreferenced = true

# FULLY_QUALIFIED_PATH_WITHOUT_USE：默认每个出现点都告警。设为 N 后，
# 同一 crate 的全限定路径至少出现 N 次才告警（过滤单次使用场景）。
[fully_qualified_path_without_use]
min_occurrences = 2
```

## 各 lint 说明

### TEST_ONLY_DEAD_CODE

rustc 的死代码分析将所有 `pub` 项视为已使用（可能被外部导出），测试代码的引用也会消音死代码告警。因此“仅被 `#[cfg(test)]`/`#[test]` 代码或 `pub use` 再导出使用”的公开项对发布产物而言是死代码，本 lint 将其找出。

已知限制：

- 同 workspace 其他 crate 的使用不可见，完全未引用的项可能误报（可用 `ignore_unreferenced = true` 缓解，或在调用点加 `#[allow]`）。
- 宏展开内的引用被保守地视为真实使用，仅被宏引用的项不会被告警。

### SCATTERED_USE_STATEMENT

检查模块级 `use` 是否出现在任何非 `use` 项之后，以及函数体内局部 `use`。这两种模式在 AI 生成代码中常见，会让模块依赖难以一览。

### FULLY_QUALIFIED_PATH_WITHOUT_USE

检查以外部 crate 名（如 `std::fmt::Debug`）或 `crate::` 开头、且至少两段的全限定路径在调用点内联出现。AI 生成代码倾向在每个使用点写全路径，使调用点冗长并掩盖模块真实依赖。

## 目录结构

- `src/` — 三个 lint 的实现，`lib.rs` 负责注册
- `ui/` — UI 测试样例
- `local_patches/dylint_linting` — 对 `dylint_linting` crate 的本地补丁（适配新版 nightly 的 `rustc_lint` API）
