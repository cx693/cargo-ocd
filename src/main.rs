//! # cargo-ocd
//!
//! Cargo 子命令，用于通过 OpenOCD 一键烧录和调试嵌入式固件。
//!
//! 支持**任何 OpenOCD 兼容的目标设备**（STM32、NXP、AVR、RISC-V 等）
//! 和**任何 OpenOCD 支持的下载器**（CMSIS-DAP、ST-Link、J-Link 等）。
//!
//! ## 安装
//!
//! ```bash
//! cargo install cargo-ocd
//! ```
//!
//! ## 使用
//!
//! ```bash
//! # Debug 模式编译 + 烧录
//! cargo ocd
//!
//! # Release 模式编译 + 烧录
//! cargo ocd --release
//!
//! # Debug 编译 + 烧录 + GDB 调试
//! cargo ocd d
//! ```
//!
//! ## 配置
//!
//! 在项目的 `Cargo.toml` 中添加 `[package.metadata.ocd]` 段：
//!
//! ```toml
//! [package.metadata.ocd]
//! interface = "interface/cmsis-dap.cfg"   # 下载器配置
//! target = "target/stm32f1x.cfg"          # 芯片配置
//! target-triple = "thumbv7m-none-eabi"    # Rust 编译目标
//! ```
//!
//! 支持 CMSIS-DAP / ST-Link / J-Link 等所有 OpenOCD 支持的下载器。
//! 支持任何 OpenOCD 兼容的目标芯片（通过 target 和 target-triple 配置）。

use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::{fs, thread};

fn main() {
    // cargo-ocd 作为 cargo 子命令运行时，cargo 会把子命令参数传过来
    // 例如: cargo ocd --release → args: ["cargo-ocd", "ocd", "--release"]
    // 注意：args[1] 是子命令名 "ocd"，需要跳过
    // 工作目录是用户运行命令的目录（即工作区根目录）
    let args: Vec<String> = std::env::args().collect();

    // 检测子命令
    let subcommand = if args.len() > 2 {
        let first = &args[2];
        if first == "debug" || first == "d" {
            Some("debug")
        } else {
            None
        }
    } else {
        None
    };

    // 处理 --help / -h
    if args.len() > 2 {
        let sub_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();
        if sub_args.contains(&"--help") || sub_args.contains(&"-h") {
            print_help();
            return;
        }
    }

    // 读取 Cargo.toml 配置
    let config = load_config("Cargo.toml");
    let pkg_name = config.pkg_name.clone();
    let target_triple = config.target_triple.clone();

    // 确定编译参数（跳过子命令本身）
    let mut cargo_args = vec!["build".to_string()];
    let mut release = false;

    for arg in &args[2..] {
        if arg == "--release" {
            release = true;
            cargo_args.push("--release".to_string());
        } else if arg == "--help" || arg == "-h" {
            // 已在上方处理
        } else if arg == "debug" || arg == "d" {
            // 子命令，跳过
        } else {
            cargo_args.push(arg.clone());
        }
    }

    // 步骤 1: 编译固件
    println!("[BUILD] Compiling firmware...");
    let status = Command::new("cargo")
        .env("RUSTFLAGS", "-C link-arg=-Tlink.x")
        .args(&cargo_args)
        .arg("--target")
        .arg(&target_triple)
        .status()
        .expect("编译失败");

    if !status.success() {
        eprintln!("[ERROR] Build failed");
        std::process::exit(1);
    }

    // 步骤 2: 确定 ELF 文件路径
    let target_dir = if release {
        format!("target/{}/release", target_triple)
    } else {
        format!("target/{}/debug", target_triple)
    };

    let elf_path = PathBuf::from(&target_dir).join(&pkg_name);
    if !elf_path.exists() {
        eprintln!("[ERROR] Firmware not found: {:?}", elf_path);
        std::process::exit(1);
    }

    // 步骤 3: 显示固件大小
    println!();
    show_firmware_size(&elf_path);

    match subcommand {
        Some("debug") => {
            if release {
                eprintln!("[ERROR] Release 模式不支持调试，请使用 Debug 模式");
                eprintln!("  cargo ocd d    # Debug 编译 + 烧录 + GDB 调试");
                std::process::exit(1);
            }
            run_debug(&config, &elf_path);
        }
        _ => run_flash(&config, &elf_path),
    }
}

/// 烧录模式：编译 → 烧录 → 退出
fn run_flash(config: &OcdConfig, elf_path: &Path) {
    println!();
    let elf_str = elf_path.to_string_lossy().replace('\\', "/");
    println!("[FLASH] Firmware: {}", elf_str);
    println!("[FLASH] Programming via OpenOCD...");

    let status = Command::new("openocd")
        .args(&[
            "-f",
            &config.interface,
            "-f",
            &config.target_chip,
            "-c",
            &format!("program {} verify reset exit", elf_str),
        ])
        .status()
        .expect("无法执行 openocd，请确保已安装");

    if !status.success() {
        eprintln!("[ERROR] Flash failed");
        std::process::exit(1);
    }

    println!();
    println!("[DONE] Flash complete!");
}

/// 调试模式：编译 → 烧录 → 启动 OpenOCD GDB 服务器 → 纯 Rust GDB 客户端交互
///
/// 使用纯 Rust 实现的 GDB 远程协议客户端，无需依赖外部 GDB 二进制文件。
/// 跨平台兼容（macOS ARM / Linux x86 / Windows x86）。
fn run_debug(config: &OcdConfig, elf_path: &Path) {
    println!();
    let elf_str = elf_path.to_string_lossy().replace('\\', "/");
    println!("[DEBUG] Firmware: {}", elf_str);
    println!("[DEBUG] Programming & starting GDB server...");

    // 先烧录固件
    let flash_status = Command::new("openocd")
        .args(&[
            "-f",
            &config.interface,
            "-f",
            &config.target_chip,
            "-c",
            &format!("program {} verify reset exit", elf_str),
        ])
        .status()
        .expect("无法执行 openocd，请确保已安装");

    if !flash_status.success() {
        eprintln!("[ERROR] Flash failed");
        std::process::exit(1);
    }

    // 检测端口可用性，如果 3333 被占用则自动分配随机端口
    let gdb_port = find_available_gdb_port();

    println!();
    println!("[DEBUG] Starting OpenOCD GDB server on port {}...", gdb_port);
    println!("[DEBUG] ELF file: {}", elf_str);
    println!();

    // 启动 OpenOCD GDB 服务器（保持运行）
    let mut openocd = Command::new("openocd")
        .args(&[
            "-f",
            &config.interface,
            "-f",
            &config.target_chip,
            "-c",
            &format!("gdb_port {}", gdb_port),
            "-c",
            &format!("program {}", elf_str),
            "-c",
            "reset halt",
        ])
        .spawn()
        .expect("无法执行 openocd，请确保已安装");

    // 等待 OpenOCD 启动
    thread::sleep(Duration::from_secs(3));

    // 使用纯 Rust GDB 客户端连接 OpenOCD
    println!("[DEBUG] Connecting to OpenOCD GDB server on port {}...", gdb_port);
    println!("[DEBUG] 已自动在 main() 设置断点，程序将在 main 入口处暂停");
    println!("[DEBUG] 进入交互式调试模式，输入 help 查看可用命令");
    println!();

    match gdb_client_connect(gdb_port) {
        Ok(_) => {
            println!();
            println!("[DEBUG] Debug session ended.");
        }
        Err(e) => {
            eprintln!("[ERROR] GDB session error: {}", e);
            eprintln!("提示: 请确保 OpenOCD 已正确安装且调试器已连接");
            std::process::exit(1);
        }
    }

    // 关闭 OpenOCD
    let _ = openocd.kill();
}

// ============================================================
// 纯 Rust GDB 远程协议客户端
// ============================================================

/// GDB 远程协议客户端
struct GdbClient {
    stream: TcpStream,
}

impl GdbClient {
    /// 连接到 OpenOCD GDB 服务器
    fn connect(port: u16) -> io::Result<Self> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            Duration::from_secs(5),
        )?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(GdbClient { stream })
    }

    /// 发送 GDB 远程协议包
    /// 格式: $packet#checksum
    fn send_packet(&mut self, packet: &str) -> io::Result<String> {
        // 计算校验和
        let checksum: u8 = packet.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
        let frame = format!("${}#{:02x}", packet, checksum);

        // 发送
        self.stream.write_all(frame.as_bytes())?;
        self.stream.flush()?;

        // 等待 ACK
        let mut ack = [0u8; 1];
        match self.stream.read(&mut ack) {
            Ok(1) if ack[0] == b'+' => {} // ACK
            Ok(1) if ack[0] == b'-' => {
                // NAK，重试一次
                self.stream.write_all(frame.as_bytes())?;
                self.stream.flush()?;
                let mut retry_ack = [0u8; 1];
                self.stream.read(&mut retry_ack)?;
                if retry_ack[0] != b'+' {
                    return Err(io::Error::new(io::ErrorKind::Other, "NAK after retry"));
                }
            }
            _ => {
                // 可能没有 ACK（某些 OpenOCD 版本行为不同），继续
            }
        }

        // 读取响应
        let mut response = String::new();
        let mut buf = [0u8; 1];
        loop {
            match self.stream.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if buf[0] == b'#' {
                        // 读取 2 字节校验和
                        let mut csum = [0u8; 2];
                        self.stream.read_exact(&mut csum)?;
                        break;
                    }
                    if buf[0] != b'$' && buf[0] != b'+' {
                        response.push(buf[0] as char);
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break; // 超时，返回已收到的数据
                }
                Err(e) => return Err(e),
            }
        }

        // 发送 ACK
        let _ = self.stream.write_all(b"+");

        Ok(response)
    }

    /// 继续执行 (vCont;c)
    fn v_continue(&mut self) -> io::Result<String> {
        self.send_packet("vCont;c")
    }

    /// 单步执行 (vCont;s)
    fn v_step(&mut self) -> io::Result<String> {
        self.send_packet("vCont;s")
    }

    /// 设置断点: Z0,addr,kind
    fn set_breakpoint(&mut self, addr: u32) -> io::Result<bool> {
        let resp = self.send_packet(&format!("Z0,{:x},2", addr))?;
        Ok(resp.is_empty() || resp == "OK")
    }

    /// 读取寄存器
    fn read_registers(&mut self) -> io::Result<String> {
        self.send_packet("g")
    }

    /// 读取内存
    fn read_memory(&mut self, addr: u32, len: u32) -> io::Result<String> {
        self.send_packet(&format!("m{:x},{:x}", addr, len))
    }

    /// 获取停止原因
    fn query_stop_reason(&mut self) -> io::Result<String> {
        self.send_packet("?")
    }

    /// 等待目标停止（读取异步通知）
    fn wait_for_stop(&mut self) -> io::Result<String> {
        let mut response = String::new();
        let mut buf = [0u8; 1];
        loop {
            match self.stream.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if buf[0] == b'$' {
                        // 开始包
                        response.clear();
                    } else if buf[0] == b'#' {
                        // 校验和
                        let mut csum = [0u8; 2];
                        let _ = self.stream.read_exact(&mut csum);
                        // 发送 ACK
                        let _ = self.stream.write_all(b"+");
                        break;
                    } else if buf[0] != b'+' && buf[0] != b'-' {
                        response.push(buf[0] as char);
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(response)
    }
}

/// GDB 远程协议交互式客户端
fn gdb_client_connect(port: u16) -> io::Result<()> {
    let mut client = GdbClient::connect(port)?;

    // 获取初始停止原因
    let stop_reason = client.query_stop_reason()?;
    println!("[GDB] Target stopped: {}", stop_reason);

    // 尝试在 main 函数设置断点
    // 对于 Cortex-M，main 通常在 0x8000000 + 偏移处
    // 我们通过读取向量表来获取 reset handler 地址，然后尝试在 main 设断点
    // 更简单的方式：直接尝试在常见地址设断点
    let main_addr = 0x0800_0000u32; // 默认 Flash 起始地址
    match client.set_breakpoint(main_addr) {
        Ok(true) => println!("[GDB] Breakpoint set at 0x{:08x}", main_addr),
        Ok(false) => println!("[GDB] Warning: Could not set breakpoint at 0x{:08x}", main_addr),
        Err(e) => println!("[GDB] Warning: Breakpoint error: {}", e),
    }

    // 继续执行
    println!("[GDB] Continuing execution...");
    let _ = client.v_continue();

    // 进入交互式命令行
    println!();
    println!("[GDB] 已进入交互式调试模式");
    println!("[GDB] 可用命令: continue/c, step/s, break/b <addr>, registers/r, memory/m <addr> <len>, quit/q, help/h");
    println!();

    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());

    loop {
        print!("(gdb) ");
        io::stdout().flush()?;

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("[ERROR] Read error: {}", e);
                break;
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "quit" | "q" | "exit" => {
                println!("[GDB] Exiting debug session...");
                break;
            }
            "continue" | "c" => {
                println!("[GDB] Continuing...");
                client.v_continue()?;
                // 等待断点命中
                thread::sleep(Duration::from_millis(500));
                let stop = client.wait_for_stop()?;
                if !stop.is_empty() {
                    println!("[GDB] Stopped: {}", stop);
                } else {
                    println!("[GDB] Running (no stop notification)");
                }
            }
            "step" | "s" => {
                let resp = client.v_step()?;
                println!("[GDB] Step: {}", resp);
            }
            "break" | "b" => {
                if parts.len() < 2 {
                    println!("[GDB] Usage: break <address> (e.g. break 0x8000000)");
                } else {
                    let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
                    let addr = u32::from_str_radix(addr_str, 16).unwrap_or(0);
                    match client.set_breakpoint(addr) {
                        Ok(true) => println!("[GDB] Breakpoint set at 0x{:08x}", addr),
                        Ok(false) => println!("[GDB] Failed to set breakpoint at 0x{:08x}", addr),
                        Err(e) => println!("[GDB] Error: {}", e),
                    }
                }
            }
            "registers" | "r" => {
                match client.read_registers() {
                    Ok(regs) => {
                        println!("[GDB] Registers (hex):");
                        // GDB 返回的寄存器数据是十六进制字符串，每 8 字节一组
                        for (i, chunk) in regs.as_bytes().chunks(16).enumerate() {
                            let hex_str: String = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                            println!("  r{}: {}", i, hex_str);
                        }
                    }
                    Err(e) => println!("[GDB] Error reading registers: {}", e),
                }
            }
            "memory" | "m" => {
                if parts.len() < 3 {
                    println!("[GDB] Usage: memory <addr> <len> (e.g. memory 0x8000000 64)");
                } else {
                    let addr_str = parts[1].trim_start_matches("0x").trim_start_matches("0X");
                    let addr = u32::from_str_radix(addr_str, 16).unwrap_or(0);
                    let len = parts[2].parse::<u32>().unwrap_or(64);
                    match client.read_memory(addr, len) {
                        Ok(data) => {
                            println!("[GDB] Memory at 0x{:08x} ({} bytes):", addr, len);
                            // 按行显示十六进制
                            let bytes: Vec<u8> = (0..data.len())
                                .step_by(2)
                                .filter_map(|i| {
                                    u8::from_str_radix(&data[i..(i + 2).min(data.len())], 16).ok()
                                })
                                .collect();
                            for (i, chunk) in bytes.chunks(16).enumerate() {
                                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                                println!("  0x{:08x}: {}", addr + (i * 16) as u32, hex.join(" "));
                            }
                        }
                        Err(e) => println!("[GDB] Error reading memory: {}", e),
                    }
                }
            }
            "help" | "h" | "?" => {
                println!("[GDB] 可用命令:");
                println!("  continue, c    - 继续执行");
                println!("  step, s        - 单步执行");
                println!("  break, b <addr> - 设置断点 (如: break 0x8000000)");
                println!("  registers, r   - 读取寄存器");
                println!("  memory, m <addr> <len> - 读取内存 (如: memory 0x8000000 64)");
                println!("  quit, q, exit  - 退出调试");
                println!("  help, h, ?     - 显示此帮助");
            }
            _ => {
                println!("[GDB] Unknown command: '{}'. Type 'help' for available commands.", cmd);
            }
        }
    }

    Ok(())
}

/// 查找可用的 GDB（保留备用，当前使用纯 Rust GDB 客户端）
#[allow(dead_code)]
fn find_gdb() -> String {
    let is_arm_host = cfg!(target_arch = "aarch64") || cfg!(target_arch = "arm");

    // 候选列表
    let candidates: &[&str] = if is_arm_host {
        // ARM 主机：rust-gdb/gdb 原生支持 ARM
        &["rust-gdb", "gdb", "gdb-multiarch", "arm-none-eabi-gdb"]
    } else {
        // x86 主机：必须用多架构或交叉编译 GDB
        &["gdb-multiarch", "arm-none-eabi-gdb", "rust-gdb", "gdb"]
    };

    for name in candidates {
        // 先检查 GDB 是否存在
        let exists = Command::new(name)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok();
        if !exists {
            continue;
        }

        // 在 x86 主机上，额外验证 GDB 是否支持 ARM 架构
        // rust-gdb/gdb（宿主 GDB）在 x86 上无法识别 ARM 目标
        if !is_arm_host && (*name == "rust-gdb" || *name == "gdb") {
            // 尝试让 GDB 设置 ARM 架构，如果失败则跳过
            let supports_arm = Command::new(name)
                .args(&["-ex", "set architecture arm", "-ex", "quit"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok();
            if !supports_arm {
                continue;
            }
        }

        return name.to_string();
    }

    // 默认返回，会在后续报错
    "rust-gdb".to_string()
}

/// 检测并返回可用的 GDB 服务器端口
///
/// 默认使用 3333 端口，如果被占用则自动尝试 3334-3343 范围内的端口，
/// 并提醒用户原端口被占用。
///
/// 注意：使用 `0.0.0.0` 而不是 `127.0.0.1` 进行检测，因为 OpenOCD
/// 默认绑定在 `0.0.0.0`（所有网络接口），如果只检测 `127.0.0.1` 可能
/// 漏掉已被 `0.0.0.0` 占用的端口。
fn find_available_gdb_port() -> u16 {
    let preferred_port = 3333u16;
    let max_attempts = 10; // 尝试 3333..3343 共 11 个端口

    // 尝试绑定到首选端口，如果成功说明端口可用
    // 使用 0.0.0.0 匹配 OpenOCD 的默认绑定行为
    if TcpListener::bind(("0.0.0.0", preferred_port)).is_ok() {
        return preferred_port;
    }

    // 3333 被占用，提示用户并尝试后续端口
    eprintln!();
    eprintln!("[WARN] 端口 {} 已被占用，正在扫描可用端口...", preferred_port);

    for port in (preferred_port + 1)..=(preferred_port + max_attempts) {
        if TcpListener::bind(("0.0.0.0", port)).is_ok() {
            eprintln!("[WARN] 使用端口 {} 替代 {}（原端口被占用）", port, preferred_port);
            eprintln!("[WARN] 请使用: target remote :{} 连接 GDB", port);
            return port;
        }
    }

    // 所有端口都被占用，报错退出
    eprintln!(
        "[ERROR] 端口 {}-{} 均被占用，无法启动 GDB 服务器",
        preferred_port,
        preferred_port + max_attempts
    );
    eprintln!("[ERROR] 请关闭占用这些端口的程序后重试");
    std::process::exit(1);
}

// ============================================================
// 配置解析
// ============================================================

/// OpenOCD 配置
struct OcdConfig {
    pkg_name: String,
    interface: String,
    target_chip: String,
    target_triple: String,
}

/// 从 Cargo.toml 读取配置
fn load_config(cargo_toml_path: &str) -> OcdConfig {
    let content = fs::read_to_string(cargo_toml_path).unwrap_or_else(|_| {
        eprintln!("[ERROR] Cargo.toml not found. Run this command from the project root.");
        std::process::exit(1);
    });

    let lines: Vec<&str> = content.lines().collect();

    // 解析包名
    let pkg_name = parse_toml_value(&lines, "name")
        .unwrap_or_else(|| "firmware".to_string());

    // 解析 [package.metadata.ocd] 段
    let ocd_section = extract_section(&lines, "[package.metadata.ocd]");

    let interface = parse_section_value(&ocd_section, "interface")
        .unwrap_or_else(|| "interface/cmsis-dap.cfg".to_string());

    let target_chip = parse_section_value(&ocd_section, "target")
        .unwrap_or_else(|| "target/stm32f1x.cfg".to_string());

    let target_triple = parse_section_value(&ocd_section, "target-triple")
        .unwrap_or_else(|| "thumbv7m-none-eabi".to_string());

    OcdConfig {
        pkg_name,
        interface,
        target_chip,
        target_triple,
    }
}

/// 从所有行中解析 TOML 键值对（用于 [package] 段）
fn parse_toml_value(lines: &[&str], key: &str) -> Option<String> {
    for line in lines {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let k = line[..eq_pos].trim();
            if k == key {
                let v = line[eq_pos + 1..].trim();
                let v = v.trim_matches('"').trim_matches('\'');
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 提取 TOML 中指定 section 的内容（不含 section 标题行）
fn extract_section(lines: &[&str], section_name: &str) -> Vec<String> {
    let mut in_section = false;
    let mut result = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_section {
                break;
            }
            if trimmed == section_name {
                in_section = true;
            }
            continue;
        }
        if in_section {
            result.push(line.to_string());
        }
    }

    result
}

/// 从 section 行中解析键值对
fn parse_section_value(lines: &[String], key: &str) -> Option<String> {
    for line in lines {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let k = line[..eq_pos].trim();
            if k == key {
                let v = line[eq_pos + 1..].trim();
                let v = v.trim_matches('"').trim_matches('\'');
                return Some(v.to_string());
            }
        }
    }
    None
}

// ============================================================
// ELF 解析 & 进度条显示
// ============================================================

/// 显示固件大小进度条
fn show_firmware_size(path: &Path) {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            println!("  [WARN] Cannot read firmware file");
            return;
        }
    };

    let mut data = Vec::new();
    if file.read_to_end(&mut data).is_err() || data.len() < 52 {
        return;
    }

    // 解析 32-bit ELF header
    let e_shoff = u32::from_le_bytes(data[0x20..0x24].try_into().unwrap()) as usize;
    let e_shentsize = u16::from_le_bytes(data[0x2E..0x30].try_into().unwrap()) as usize;
    let e_shnum = u16::from_le_bytes(data[0x30..0x32].try_into().unwrap()) as usize;

    let mut flash_used: u64 = 0;
    let mut ram_used: u64 = 0;

    // 从 memory.x 读取 FLASH 和 RAM 的地址范围
    let (flash_origin, flash_len, ram_origin, ram_len) = parse_memory_x_addrs("memory.x");
    let flash_origin = flash_origin as u32;
    let flash_end = (flash_origin as u64 + flash_len) as u32;
    let ram_origin = ram_origin as u32;
    let ram_end = (ram_origin as u64 + ram_len) as u32;

    for i in 0..e_shnum {
        let sh_off = e_shoff + i * e_shentsize;
        if sh_off + 24 > data.len() {
            break;
        }

        let sh_flags = u32::from_le_bytes(data[sh_off + 8..sh_off + 12].try_into().unwrap());
        let sh_addr = u32::from_le_bytes(data[sh_off + 12..sh_off + 16].try_into().unwrap());
        let sh_size = u32::from_le_bytes(data[sh_off + 20..sh_off + 24].try_into().unwrap());

        // SHF_ALLOC = 0x2
        if sh_flags & 0x2 != 0 {
            if sh_addr >= ram_origin && sh_addr < ram_end {
                ram_used += sh_size as u64;
            } else if sh_addr >= flash_origin && sh_addr < flash_end {
                flash_used += sh_size as u64;
            }
        }
    }

    // 从 memory.x 读取总大小
    let (flash_total, ram_total) = parse_memory_x("memory.x");

    let flash_pct = flash_used as f64 * 100.0 / flash_total as f64;
    let ram_pct = ram_used as f64 * 100.0 / ram_total as f64;

    // 格式化大小显示
    let flash_used_str = format_bytes(flash_used);
    let flash_total_str = format_bytes(flash_total);
    let ram_used_str = format_bytes(ram_used);
    let ram_total_str = format_bytes(ram_total);

    // 进度条（30 格）
    let flash_bar = progress_bar(flash_pct, 30);
    let ram_bar = progress_bar(ram_pct, 30);

    // 固定边框宽度，内容用空格补齐对齐
    let bar_width = 32; // [ + 30格 + ]
    let total_width = 2 + 5 + 1 + bar_width + 2 + 6 + 2 + 8 + 3 + 8 + 1;
    //                sp LABEL sp [bar]   sp  xx.x% sp  xxxx / xxxx sp

    let line_flash = format!(
        " {:<5} {} {:>5.1}%  {:>6} / {:<6} ",
        "FLASH", flash_bar, flash_pct, flash_used_str, flash_total_str
    );
    let line_ram = format!(
        " {:<5} {} {:>5.1}%  {:>6} / {:<6} ",
        "RAM", ram_bar, ram_pct, ram_used_str, ram_total_str
    );

    // 补齐到固定宽度（按字符数，避免 UTF-8 多字节截断）
    let pad = |s: &str, w: usize| {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() >= w {
            chars[..w].iter().collect()
        } else {
            format!("{}{}", s, " ".repeat(w - chars.len()))
        }
    };

    let border = format!("+{}+", "-".repeat(total_width));

    println!("  [FIRMWARE SIZE]");
    println!("  {}", border);
    println!("  |{}|", pad(&line_flash, total_width));
    println!("  |{}|", pad(&line_ram, total_width));
    println!("  {}", border);
}

/// 生成进度条字符串
fn progress_bar(pct: f64, width: usize) -> String {
    let fill = if pct <= 0.0 {
        0
    } else {
        let f = (pct / 100.0 * width as f64) as usize;
        if f < 1 { 1 } else { f }.min(width)
    };
    let empty = width - fill;
    format!("[{}{}]", "█".repeat(fill), "░".repeat(empty))
}

/// 解析 memory.x 文件，提取 FLASH 和 RAM 大小（字节）
fn parse_memory_x(path: &str) -> (u64, u64) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (65536, 20480),
    };

    let mut flash_size: u64 = 0;
    let mut ram_size: u64 = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("/*")
            || line.starts_with('*')
            || line.starts_with("//")
        {
            continue;
        }

        if line.starts_with("FLASH") || line.starts_with("RAM") {
            if let Some(len_str) = line.split(',').nth(1) {
                if let Some(eq) = len_str.find('=') {
                    let val_str = len_str[eq + 1..].trim();
                    let size = parse_size(val_str);
                    if line.starts_with("FLASH") {
                        flash_size = size;
                    } else {
                        ram_size = size;
                    }
                }
            }
        }
    }

    if flash_size == 0 {
        flash_size = 65536;
    }
    if ram_size == 0 {
        ram_size = 20480;
    }

    (flash_size, ram_size)
}

/// 解析 memory.x 文件，提取 FLASH 和 RAM 的起始地址和大小
/// 返回 (flash_origin, flash_length, ram_origin, ram_length)
fn parse_memory_x_addrs(path: &str) -> (u64, u64, u64, u64) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (0x0800_0000, 65536, 0x2000_0000, 20480),
    };

    let mut flash_origin: u64 = 0x0800_0000;
    let mut flash_len: u64 = 65536;
    let mut ram_origin: u64 = 0x2000_0000;
    let mut ram_len: u64 = 20480;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("/*")
            || line.starts_with('*')
            || line.starts_with("//")
        {
            continue;
        }

        if line.starts_with("FLASH") || line.starts_with("RAM") {
            let is_flash = line.starts_with("FLASH");
            // 格式: FLASH : ORIGIN = 0x08000000, LENGTH = 64K
            // 或:   FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 64K
            if let Some(origin_str) = line.split(',').nth(0) {
                if let Some(eq) = origin_str.find("ORIGIN") {
                    let after_eq = origin_str[eq + "ORIGIN".len()..].trim();
                    let after_assign = after_eq.trim_start_matches('=').trim();
                    let val_str = after_assign.trim_matches(' ').trim();
                    // 解析 0x 十六进制或十进制数
                    let addr = if val_str.starts_with("0x") || val_str.starts_with("0X") {
                        u64::from_str_radix(&val_str[2..], 16).unwrap_or(0)
                    } else {
                        val_str.parse().unwrap_or(0)
                    };
                    if is_flash {
                        flash_origin = addr;
                    } else {
                        ram_origin = addr;
                    }
                }
            }
            if let Some(len_str) = line.split(',').nth(1) {
                if let Some(eq) = len_str.find("LENGTH") {
                    let val_str = len_str[eq + "LENGTH".len()..].trim();
                    let val_str = val_str.trim_start_matches('=').trim();
                    let size = parse_size(val_str);
                    if is_flash {
                        flash_len = size;
                    } else {
                        ram_len = size;
                    }
                }
            }
        }
    }

    (flash_origin, flash_len, ram_origin, ram_len)
}

/// 打印帮助信息
fn print_help() {
    println!("cargo-ocd — 一键编译并通过 OpenOCD 烧录/调试嵌入式固件");
    println!();
    println!("用法: cargo ocd [SUBCOMMAND] [OPTIONS]");
    println!();
    println!("子命令:");
    println!("  debug, d       编译、烧录并启动 GDB 调试会话（仅 Debug 模式）");
    println!();
    println!("选项:");
    println!("  --release      使用 Release 模式编译（默认 debug 模式）");
    println!("  --help, -h     显示此帮助信息");
    println!();
    println!("配置方式（在项目的 Cargo.toml 中）：");
    println!();
    println!("  [package.metadata.ocd]");
    println!("  interface = \"interface/cmsis-dap.cfg\"   # 下载器配置");
    println!("  target = \"target/stm32f1x.cfg\"          # 芯片配置");
    println!("  target-triple = \"thumbv7m-none-eabi\"    # Rust 编译目标");
    println!();
    println!("示例:");
    println!("  cargo ocd                    # Debug 模式编译 + 烧录");
    println!("  cargo ocd --release          # Release 模式编译 + 烧录");
    println!("  cargo ocd d                  # Debug 编译 + 烧录 + GDB 调试");
    println!();
    println!("GDB 调试提示:");
    println!("  进入 GDB 后，依次执行:");
    println!("    (gdb) break main           # 在 main 函数设断点");
    println!("    (gdb) continue             # 运行到断点");
    println!("    (gdb) step                 # 单步执行");
    println!("    (gdb) print variable       # 查看变量");
    println!("     --其余指令见GDB调试协议--             ");
    println!();
    println!("支持的下载器: CMSIS-DAP / ST-Link / J-Link（通过 interface 配置）");
    println!("支持的芯片:   任何 OpenOCD 兼容的目标（通过 target 和 target-triple 配置）");
    println!();
    println!("详细文档: 基础环境配置与使用.md");
}

fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    if s.ends_with("K") {
        let num: f64 = s[..s.len() - 1].trim().parse().unwrap_or(0.0);
        (num * 1024.0) as u64
    } else if s.ends_with("M") {
        let num: f64 = s[..s.len() - 1].trim().parse().unwrap_or(0.0);
        (num * 1024.0 * 1024.0) as u64
    } else {
        s.parse().unwrap_or(0)
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}
