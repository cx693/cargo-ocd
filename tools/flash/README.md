# cargo-ocd

[![Crates.io](https://img.shields.io/crates/v/cargo-ocd)](https://crates.io/crates/cargo-ocd)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](https://github.com/CXi/stm32f103_blink)

**cargo-ocd** 是一个 Cargo 子命令，用于通过 OpenOCD 一键烧录和调试 STM32 固件。

只需 `cargo ocd` 即可完成编译 + 烧录，无需手动敲 OpenOCD 命令。

---

## 功能特性

- ✅ **一键烧录**：`cargo ocd` — 编译 + 烧录一步到位
- ✅ **Release 模式**：`cargo ocd --release` — 优化编译后烧录
- ✅ **GDB 调试**：`cargo ocd d` — 编译 + 烧录 + 自动启动 GDB 调试会话
- ✅ **全系列 STM32 支持**：通过 `memory.x` 动态解析 FLASH/RAM 地址范围
- ✅ **固件大小显示**：烧录前自动显示 FLASH/RAM 占用进度条
- ✅ **下载器无关**：支持 CMSIS-DAP / ST-Link / J-Link 等所有 OpenOCD 支持的下载器
- ✅ **跨平台**：Windows / Linux / macOS 全平台支持

## 依赖项

使用前需要安装：

| 工具 | 用途 | 安装方式 |
|------|------|----------|
| [OpenOCD](https://openocd.org/) | 烧录和 GDB 服务器 | `brew install openocd` (macOS) / `apt install openocd` (Linux) / [官网下载](https://openocd.org/) (Windows) |
| [Rust](https://www.rust-lang.org/) | 编译工具链 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| GDB（可选，调试用） | 断点调试 | `rustup component add rust-gdb`（推荐，Rust 自带） |

## 安装

```bash
cargo install cargo-ocd
```

## 快速开始

### 1. 配置 Cargo.toml

在项目的 `Cargo.toml` 中添加烧录配置：

```toml
[package.metadata.ocd]
# 下载器接口配置文件
#   CMSIS-DAP:  interface/cmsis-dap.cfg
#   ST-Link:    interface/stlink.cfg
#   J-Link:     interface/jlink.cfg
interface = "interface/cmsis-dap.cfg"

# 目标芯片配置文件
#   STM32F1:  target/stm32f1x.cfg
#   STM32F4:  target/stm32f4x.cfg
#   更多参考 OpenOCD 文档
target = "target/stm32f1x.cfg"

# Rust 编译目标
#   Cortex-M3 (STM32F1/F2):  thumbv7m-none-eabi
#   Cortex-M4 (STM32F3/F4):  thumbv7em-none-eabi
target-triple = "thumbv7m-none-eabi"
```

### 2. 烧录固件

```bash
# Debug 模式编译 + 烧录
cargo ocd

# Release 模式编译 + 烧录
cargo ocd --release
```

### 3. GDB 调试

```bash
# 编译 + 烧录 + 启动 GDB 调试（仅 Debug 模式）
cargo ocd d
```

进入 GDB 后会自动在 `main()` 函数设置断点并暂停，之后可使用 `step`、`next`、`print` 等命令调试。

## 子命令

| 子命令 | 说明 |
|--------|------|
| `flash`（默认） | 编译并烧录固件到芯片 |
| `debug` / `d` | 编译、烧录并启动 GDB 调试会话（仅 Debug 模式） |

## 选项

| 选项 | 说明 |
|------|------|
| `--release` | 使用 Release 模式编译（默认 Debug 模式） |
| `--help` / `-h` | 显示帮助信息 |

## 配置示例

### STM32F103C8T6（Blue Pill）

```toml
[package.metadata.ocd]
interface = "interface/cmsis-dap.cfg"
target = "target/stm32f1x.cfg"
target-triple = "thumbv7m-none-eabi"
```

### STM32F407VGT6（Discovery）

```toml
[package.metadata.ocd]
interface = "interface/stlink.cfg"
target = "target/stm32f4x.cfg"
target-triple = "thumbv7em-none-eabihf"
```

### STM32G070RBT6

```toml
[package.metadata.ocd]
interface = "interface/cmsis-dap.cfg"
target = "target/stm32g0x.cfg"
target-triple = "thumbv6m-none-eabi"
```

## 工作原理

1. `cargo ocd` 读取 `Cargo.toml` 中的 `[package.metadata.ocd]` 配置
2. 调用 `cargo build --target <target-triple>` 编译固件
3. 解析生成的 ELF 文件，显示 FLASH/RAM 占用进度条
4. 调用 OpenOCD 通过指定的下载器将固件烧录到芯片
5. 调试模式下额外启动 OpenOCD GDB 服务器并自动连接 GDB

## 许可证

本项目采用 **MIT OR Apache-2.0** 双许可证，详见项目根目录的 LICENSE 文件。
