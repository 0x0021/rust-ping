# rust-ping

<img src="assets/icon.png" alt="rust-ping icon" width="128" align="right" />

[![CI](https://github.com/0x0021/rust-ping/actions/workflows/ci.yml/badge.svg)](https://github.com/0x0021/rust-ping/actions/workflows/ci.yml)
[![Release](https://github.com/0x0021/rust-ping/actions/workflows/release.yml/badge.svg)](https://github.com/0x0021/rust-ping/actions/workflows/release.yml)

用 Rust 实现的跨平台命令行 ping 工具，基于 ICMP Echo 请求/回复。支持域名与 IP、丰富的命令行参数、TOML 配置文件、普通/颜色/表格三种输出样式，以及 Windows / macOS / Linux。

## 功能特性

- 解析域名或 IPv4/IPv6 地址（域名优先 IPv4）
- 发送 ICMP Echo 请求并接收回复，显示序号、TTL、字节数、往返时间
- 统计丢包率与最小 / 平均 / 最大延迟
- 指定次数、间隔、超时、载荷大小、TTL，或持续运行（Ctrl+C 结束）
- 命令行参数 + TOML 配置文件双重配置，命令行优先级更高
- 三种输出样式：普通文本、颜色高亮、表格；独立的颜色开关
- 统一错误类型、详细日志、网络不可达等常见故障的友好提示
- 提供 macOS `.app` bundle（含 Bundle ID），可上架 Setapp 等应用商店

## 构建与安装

需要 Rust 1.70+（含 Cargo）：

```bash
git clone https://github.com/0x0021/rust-ping.git
cd rust-ping
cargo build --release
# 产物：target/release/rust-ping
```

也可以直接通过 Cargo 安装：

```bash
cargo install --path .
```

### 预编译二进制与原生安装包

GitHub Releases 提供以下产物（均含 SHA256 校验文件）：

| 平台 | 压缩包 | 原生安装包 |
| --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `.deb`（amd64）、`.rpm`（x86_64） |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | `.deb`（arm64）、`.rpm`（aarch64） |
| macOS Intel / Apple Silicon | 两个架构各一个 tar.gz | `.pkg`（universal，含两种架构）；`.app.zip`（universal，带 Bundle ID，供商店上传） |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `.msi` |

到 [Releases](https://github.com/0x0021/rust-ping/releases) 页面下载对应文件：

```bash
# 压缩包，解压即用
tar -xzf rust-ping-v0.1.4-x86_64-unknown-linux-gnu.tar.gz
cd rust-ping-v0.1.4-x86_64-unknown-linux-gnu
./rust-ping 1.1.1.1

# 校验
sha256sum -c rust-ping-v0.1.4-x86_64-unknown-linux-gnu.tar.gz.sha256
```

原生安装包安装方式：

```bash
# Debian / Ubuntu
sudo apt install ./rust-ping-v0.1.4-amd64.deb

# RHEL / Fedora / CentOS
sudo rpm -ivh rust-ping-v0.1.4-x86_64.rpm

# macOS（universal pkg，安装到 /usr/local/bin）
sudo installer -pkg rust-ping-v0.1.4-universal-apple-darwin.pkg -target /

# Windows：双击 MSI，或命令行静默安装
msiexec /i rust-ping-v0.1.4-x86_64-pc-windows-msvc.msi /qn
```

### 自动发布

维护者推送 `v*` tag 即自动触发多平台交叉编译、原生安装包构建并创建 GitHub Release：

```bash
git tag v0.1.4
git push origin v0.1.4
```

也可以在 Actions 页面手动触发 Release 工作流并指定版本号。CI 工作流在每次 push / PR 到 `master` 时运行格式检查、clippy、测试和构建。

### 上架 macOS 应用商店（Bundle ID）

裸二进制和 `.pkg` 不含 Bundle ID，商店会报 "missing Bundle ID"。请下载 Release 中的 `rust-ping-v0.1.4-universal-apple-darwin.app.zip`——里面是一个标准 `.app` bundle，`Contents/Info.plist` 已包含：

| 键 | 值 |
| --- | --- |
| `CFBundleIdentifier`（Bundle ID） | `com.0x0021.rust-ping` |
| `CFBundleName` / `CFBundleDisplayName` | `rust-ping` |
| `CFBundleExecutable` | `rust-ping` |
| `CFBundleIconFile` | `icon.icns` |
| `CFBundleShortVersionString` / `CFBundleVersion` | 版本号（如 `0.1.4`） |
| `NSUpdateSecurityPolicy` | 已授权 Setapp Agent 更新（macOS 13+） |

校验方式：

```bash
plutil -extract CFBundleIdentifier raw -o - rust-ping.app/Contents/Info.plist
# 输出: com.0x0021.rust-ping
```

zip 顶层结构为 `rust-ping.app/` + `AppIcon.png`（无 `__MACOSX`），符合 Setapp 等商店的上传要求。上架步骤：

1. 在 Apple Developer 后台用同样的 Bundle ID 注册 App ID（首次上架后不可更改）。
2. 在商店后台创建 App 记录并填入相同的 Bundle ID。
3. 用你的 Distribution 证书重新签名（CI 产物默认未签名），再上传并公证。注意：Bundle ID 必须与签名描述文件中的 App ID 一致，否则会报 "profile does not match bundle ID"。

本地重新生成（需 macOS）：

```bash
# 生成未签名产物
packaging/macos/build-app.sh v0.1.4 target/release/rust-ping rust-ping.app rust-ping.app.zip

# 用正式证书签名后重新打包
SIGN_IDENTITY="Developer ID Application: YOUR TEAM (TEAMID)" \
  packaging/macos/build-app.sh v0.1.4 target/release/rust-ping rust-ping.app rust-ping.app.zip
```

## 用法

```bash
# 基本：ping 4 次（默认间隔 1 秒）
rust-ping 1.1.1.1

# 指定次数、间隔、载荷、超时
rust-ping example.com -c 5 -i 0.5 -s 64 -W 2

# 持续 ping，直到 Ctrl+C
rust-ping 8.8.8.8 --continuous

# 颜色高亮 / 表格输出
rust-ping 127.0.0.1 -c 3 --style color --color always
rust-ping 127.0.0.1 -c 3 --style table
# 表格样式会在 ping 结束后一次性输出完整明细表 + 汇总表（运行过程中静默）

# 使用配置文件（命令行参数可覆盖其中字段）
rust-ping --config ./config.example.toml

# 打印示例配置
rust-ping --print-config > rust-ping.toml
```

典型输出：

```text
PING 1.1.1.1 (1.1.1.1): 56 data bytes
64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.345 ms
64 bytes from 1.1.1.1: icmp_seq=2 ttl=57 time=11.892 ms
64 bytes from 1.1.1.1: icmp_seq=3 ttl=57 time=12.901 ms
--- 1.1.1.1 ping statistics ---
3 packets transmitted, 3 received, 0.0% packet loss, time 2014ms
rtt min/avg/max = 11.892/12.379/12.901 ms
```

### 命令行参数

| 参数 | 说明 | 默认值 |
| --- | --- | --- |
| `TARGET` | 目标域名或 IP | 配置文件 `target` |
| `-c, --count <N>` | 请求次数，`0` 表示持续 | `4` |
| `-i, --interval <SEC>` | 发送间隔（秒） | `1.0` |
| `-W, --timeout <SEC>` | 单次等待回复超时（秒） | `1.0` |
| `-s, --payload-size <BYTES>` | ICMP 载荷字节数 | `56` |
| `-t, --ttl <TTL>` | IPv4 TTL / IPv6 Hop Limit | `64` |
| `-L, --continuous` | 持续运行（与 `--count` 互斥） | 关闭 |
| `-v, --verbose` | 打印调试日志 | 关闭 |
| `--style <plain\|color\|table>` | 输出样式 | `plain` |
| `--color <auto\|always\|never>` | 颜色开关 | `auto` |
| `--log-level <LEVEL>` | 日志级别 | `warn` |
| `-C, --config <FILE>` | 指定配置文件 | 自动查找 |
| `--print-config` | 打印示例配置并退出 | — |

## 配置文件

不传 `--config` 时，程序依次查找 `./rust-ping.toml` 和 `~/.config/rust-ping/config.toml`，找到即加载。配置优先级为：**命令行 > 配置文件 > 内置默认值**。完整示例见 [`config.example.toml`](config.example.toml)。

```toml
target = "example.com"
count = 4
interval = 1.0
timeout = 1.0
payload_size = 56
ttl = 64
log_level = "warn"
style = "plain"      # plain / color / table
color = "auto"       # auto / always / never
```

## 平台与权限说明

程序优先创建 **DGRAM 型 ICMP socket**，在 Linux 与 macOS 上通常无需 root 即可 ping；若内核不支持，则回退到 RAW socket。

- **Linux**：DGRAM 型 ICMP 依赖 `net.ipv4.ping_group_range`（主流发行版默认已放开）。回退 RAW 时需要 `CAP_NET_RAW`，可执行 `sudo setcap cap_net_raw+ep ./rust-ping`。
- **macOS**：DGRAM 型 ICMP socket 无需 root。系统会返回带 IP 头的报文，程序已自动识别并跳过；identifier 可能被内核改写，因此回复匹配以序号为准。
- **Windows**：仅支持 RAW socket，**需要管理员权限**运行（右键「以管理员身份运行」）。Windows 上控制消息支持有限，TTL 回退显示为发送时设置的值。

使用 Clash / Surge 等代理的 TUN 模式时，域名可能解析到 `198.18.0.0/15` 段的 fake-ip。发送使用 `sendto()` 直接指定目标地址，与系统 ping 行为一致，可正常收发。

## 项目结构

```
rust-ping/
├── Cargo.toml              # 依赖与构建配置、deb/rpm 元数据
├── config.example.toml     # 示例配置文件
├── README.md
├── assets/                 # 项目图标（PNG / ICO / ICNS）
├── packaging/              # 各平台打包脚本
│   ├── macos/
│   │   ├── build-pkg.sh    # 生成 universal .pkg
│   │   └── build-app.sh    # 生成带 Bundle ID 的 .app（供商店上传）
│   └── windows/
│       └── rust-ping.wxs   # WiX v5 MSI 定义
├── .github/workflows/
│   ├── ci.yml              # fmt / clippy / test / build
│   └── release.yml         # 多平台交叉编译、安装包与 Release 发布
└── src/
    ├── main.rs             # 入口：参数解析、配置加载、日志、Ctrl+C
    ├── cli.rs              # 命令行参数定义
    ├── config.rs           # 配置结构、加载与优先级合并
    ├── icmp.rs             # ICMP 报文构造/校验、跨平台收发、TTL 提取
    ├── pinger.rs           # ping 主循环：域名解析、收发匹配、优雅退出
    ├── stats.rs            # 丢包率与延迟统计
    └── output.rs           # 普通/颜色/表格输出与颜色开关
```

## 许可证

MIT
