# cargo-ocd

[![Crates.io](https://img.shields.io/crates/v/cargo-ocd)](https://crates.io/crates/cargo-ocd)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](https://www.gnu.org/licenses/gpl-3.0.html)

**cargo-ocd** 是一个 Cargo 子命令，用于通过 OpenOCD 一键烧录和调试嵌入式固件。

**cargo-ocd** is a Cargo subcommand for one-click flashing and debugging of embedded firmware via OpenOCD.

只需 `cargo ocd` 即可完成编译 + 烧录，无需手动敲 OpenOCD 命令。

Just run `cargo ocd` to compile and flash — no need to type OpenOCD commands manually.

支持**任何 OpenOCD 兼容的目标设备**（STM32、NXP、AVR、RISC-V 等）和**任何 OpenOCD 支持的下载器**（CMSIS-DAP、ST-Link、J-Link 等）。

Supports **any OpenOCD-compatible target device** (STM32, NXP, AVR, RISC-V, etc.) and **any OpenOCD-supported debug probe** (CMSIS-DAP, ST-Link, J-Link, etc.).

---

## 参考资料 / References

> 以下文章和视频可以帮助你快速了解 cargo-ocd 的使用方法：
> The following articles and videos can help you quickly get started with cargo-ocd:

- 📖 [Cargo-OCD更新贴！ - CSDN](https://blog.csdn.net/jdjkdidjdj/article/details/160890224?sharetype=blog&shareId=160890224&sharerefer=APP&sharesource=jdjkdidjdj&sharefrom=link)
- 🎬 [RUST开发STM32！cargo-ocd也许也是一种选择！- 哔哩哔哩](https://b23.tv/y2yYcwS)

---

## 功能特性 / Features

- ✅ **一键烧录**：`cargo ocd` — 编译 + 烧录一步到位
- ✅ **One-click flash**: `cargo ocd` — compile and flash in one step
- ✅ **Release 模式**：`cargo ocd --release` — 优化编译后烧录
- ✅ **Release mode**: `cargo ocd --release` — compile with optimizations then flash
- ✅ **GDB 调试**：`cargo ocd d` — 编译 + 烧录 + 自动启动 GDB 调试会话
- ✅ **GDB debugging**: `cargo ocd d` — compile, flash, and auto-launch GDB debug session
- ✅ **全系列芯片支持**：通过 `memory.x` 动态解析 FLASH/RAM 地址范围，不限于 STM32
- ✅ **Wide chip support**: dynamically parses FLASH/RAM address ranges from `memory.x`, not limited to STM32
- ✅ **固件大小显示**：烧录前自动显示 FLASH/RAM 占用进度条
- ✅ **Firmware size display**: shows FLASH/RAM usage progress bar before flashing
- ✅ **下载器无关**：支持 CMSIS-DAP / ST-Link / J-Link 等所有 OpenOCD 支持的下载器
- ✅ **Probe agnostic**: supports CMSIS-DAP, ST-Link, J-Link, and all OpenOCD-compatible debug probes
- ✅ **跨平台**：Windows / Linux / macOS 全平台支持
- ✅ **Cross-platform**: Windows / Linux / macOS
- ✅ **ARM & RISC-V 双架构**：自动检测目标架构，选择合适的 GDB 调试器
- ✅ **ARM & RISC-V**: auto-detects target architecture and selects the appropriate GDB debugger
- ✅ **智能 GDB 选择**：根据操作系统和目标架构自动匹配最佳 GDB
- ✅ **Smart GDB selection**: automatically picks the best GDB based on OS and target architecture
- ✅ **指定 GDB 端口**：通过 `--port` 参数自定义 GDB 服务器端口（默认 3333）
- ✅ **Custom GDB port**: set GDB server port via `--port` (default 3333)

## 依赖项 / Dependencies

使用前需要安装：

Before using, you need to install:

| 工具 / Tool | 用途 / Purpose | 安装方式 / Installation |
|-------------|----------------|------------------------|
| [OpenOCD](https://openocd.org/) | 烧录和 GDB 服务器 / Flashing & GDB server | `brew install openocd` (macOS) / `apt install openocd` (Linux) / [官网下载](https://openocd.org/) (Windows) |
| [Rust](https://www.rust-lang.org/) | 编译工具链 / Compiler toolchain | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| GDB（可选，调试用 / optional, for debugging） | 断点调试 / Breakpoint debugging | 自动检测，详见下方 [GDB 选择策略](#gdb-选择策略--gdb-selection-strategy) |

## 安装 / Installation

```bash
cargo install cargo-ocd
```

## 快速开始 / Quick Start

### 1. 配置 Cargo.toml / Configure Cargo.toml

在项目的 `Cargo.toml` 中添加烧录配置：

Add the following flash configuration to your project's `Cargo.toml`:

```toml
[package.metadata.ocd]
# 下载器接口配置文件 / Debug probe interface config
#   CMSIS-DAP:  interface/cmsis-dap.cfg
#   ST-Link:    interface/stlink.cfg
#   J-Link:     interface/jlink.cfg
interface = "interface/cmsis-dap.cfg"

# 目标芯片配置文件（根据实际芯片选择）/ Target chip config (choose based on your chip)
#   STM32F1:  target/stm32f1x.cfg
#   STM32F4:  target/stm32f4x.cfg
#   NXP LPC:  target/lpc1768.cfg
#   更多参考 OpenOCD 文档 / See OpenOCD docs for more
target = "target/stm32f1x.cfg"

# Rust 编译目标（根据实际架构选择）/ Rust compilation target (choose based on your architecture)
#   Cortex-M3 (STM32F1/F2):  thumbv7m-none-eabi
#   Cortex-M4 (STM32F3/F4):  thumbv7em-none-eabi
#   RISC-V:                  riscv32imac-unknown-none-elf
target-triple = "thumbv7m-none-eabi"
```

### 2. 烧录固件 / Flash Firmware

```bash
# Debug 模式编译 + 烧录 / Debug build + flash
cargo ocd

# Release 模式编译 + 烧录 / Release build + flash
cargo ocd --release
```

### 3. GDB 调试 / GDB Debugging

```bash
# 编译 + 烧录 + 启动 GDB 调试（仅 Debug 模式）
# Build + flash + launch GDB debug session (Debug mode only)
cargo ocd d
```

进入 GDB 后会自动在 `main()` 函数设置断点并暂停，之后可使用 `step`、`next`、`print` 等命令调试。

Once inside GDB, a breakpoint will be automatically set at `main()` and execution will pause. You can then use commands like `step`, `next`, `print` for debugging.

## 子命令 / Subcommands

| 子命令 / Subcommand | 说明 / Description |
|---------------------|-------------------|
| `debug` / `d` | 编译、烧录并启动 GDB 调试会话（仅 Debug 模式）/ Build, flash, and launch GDB debug session (Debug mode only) |

## 选项 / Options

| 选项 / Option | 说明 / Description |
|---------------|-------------------|
| `--release` | 使用 Release 模式编译（默认 Debug 模式）/ Use Release mode (default is Debug) |
| `--gdb` | 调试时使用系统 GDB（macOS 可用）/ Use system GDB for debugging (macOS) |
| `--rust-gdb` | 调试时使用 rust-gdb（macOS/Linux 可用）/ Use rust-gdb for debugging (macOS/Linux) |
| `--port <PORT>` | 指定 GDB 服务器端口（默认 3333）/ Set GDB server port (default 3333) |
| `--help` / `-h` | 显示帮助信息 / Show help |

## 配置示例 / Configuration Examples

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

### RISC-V (GD32V / CH32V)

```toml
[package.metadata.ocd]
interface = "interface/cmsis-dap.cfg"
target = "target/gd32vf103.cfg"
target-triple = "riscv32imac-unknown-none-elf"
```

## GDB 选择策略 / GDB Selection Strategy

`cargo-ocd` 会根据目标架构和操作系统自动选择最合适的 GDB 调试器：

`cargo-ocd` automatically selects the most suitable GDB debugger based on the target architecture and operating system:

### ARM 架构（target-triple 以 `thumbv` / `armv` 开头）/ ARM Architecture (target-triple starts with `thumbv` / `armv`)

| 系统 / OS | GDB 优先级（从左到右）/ Priority (left to right) |
|-----------|------------------------------------------------|
| macOS | `rust-gdb` > `gdb` > `arm-none-eabi-gdb` |
| Windows | `arm-none-eabi-gdb` |
| Linux | `arm-none-eabi-gdb` |

### RISC-V 架构（target-triple 以 `riscv` 开头）/ RISC-V Architecture (target-triple starts with `riscv`)

| 系统 / OS | GDB 优先级（从左到右）/ Priority (left to right) |
|-----------|------------------------------------------------|
| macOS | `riscv64-unknown-elf-gdb` > `rust-gdb` > `gdb` |
| Windows | `riscv64-unknown-elf-gdb` |
| Linux | `riscv64-unknown-elf-gdb` |

### 手动指定 GDB / Manual GDB Selection

如果自动选择不符合需求，可以使用以下参数手动指定：

If the auto-selection doesn't meet your needs, you can manually specify GDB with these flags:

```bash
# 使用系统 GDB / Use system GDB
cargo ocd d --gdb

# 使用 rust-gdb / Use rust-gdb
cargo ocd d --rust-gdb
```

### 安装 GDB / Installing GDB

如果未找到任何可用的 GDB，程序会给出详细的安装提示：

If no GDB is found, the tool will provide detailed installation instructions:

| 系统 / OS | ARM 架构 / ARM Architecture | RISC-V 架构 / RISC-V Architecture |
|-----------|---------------------------|----------------------------------|
| macOS | `brew install gdb` | `brew install riscv64-elf-gdb` |
| Windows | [ARM GCC 工具链](https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads) | [xPack RISC-V 工具链](https://github.com/xpack-dev-tools/riscv-none-elf-gcc-xpack/releases) |
| Linux | `sudo apt install gdb-arm-none-eabi` | `sudo apt install gdb-riscv64-unknown-elf` |

## 工作原理 / How It Works

1. `cargo ocd` 读取 `Cargo.toml` 中的 `[package.metadata.ocd]` 配置 / Reads `[package.metadata.ocd]` from `Cargo.toml`
2. 调用 `cargo build --target <target-triple>` 编译固件 / Runs `cargo build --target <target-triple>` to compile the firmware
3. 解析生成的 ELF 文件，显示 FLASH/RAM 占用进度条 / Parses the generated ELF file and shows FLASH/RAM usage progress bar
4. 调用 OpenOCD 通过指定的下载器将固件烧录到芯片 / Invokes OpenOCD to flash the firmware via the specified debug probe
5. 调试模式下额外启动 OpenOCD GDB 服务器并自动连接 GDB / In debug mode, additionally starts an OpenOCD GDB server and auto-connects GDB

## 许可证 / License

本项目采用 **GNU General Public License v3.0 (GPL-3.0)**，详见项目根目录的 LICENSE 文件。

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See the LICENSE file in the project root for details.
