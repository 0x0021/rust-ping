//! 命令行参数定义。所有字段均使用 Option，便于区分「未指定」与「显式指定默认值」，
//! 从而实现 CLI > 配置文件 > 内置默认值的优先级覆盖。

use std::path::PathBuf;

use clap::Parser;

use crate::output::{ColorMode, OutputStyle};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "rust-ping",
    version,
    about = "跨平台命令行 ping 工具（ICMP Echo）",
    long_about = None,
    after_help = "示例:\n  rust-ping 1.1.1.1 -c 5 -i 0.5\n  rust-ping example.com --continuous --style table\n  rust-ping example.com --config ./rust-ping.toml"
)]
pub struct Cli {
    /// 目标域名或 IP 地址
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,

    /// 发送请求数量（0 表示持续运行）
    #[arg(short = 'c', long, value_name = "N")]
    pub count: Option<u32>,

    /// 发送间隔，单位秒（默认 1.0）
    #[arg(short = 'i', long, value_name = "SEC")]
    pub interval: Option<f64>,

    /// 单次等待回复的超时，单位秒（默认 1.0）
    #[arg(short = 'W', long, value_name = "SEC")]
    pub timeout: Option<f64>,

    /// ICMP 载荷字节数（默认 56）
    #[arg(short = 's', long, value_name = "BYTES")]
    pub payload_size: Option<usize>,

    /// TTL / IPv6 Hop Limit（默认 64）
    #[arg(short = 't', long, value_name = "TTL")]
    pub ttl: Option<u8>,

    /// 持续运行，直到 Ctrl+C
    #[arg(short = 'L', long, conflicts_with = "count")]
    pub continuous: bool,

    /// 详细输出（打印调试日志）
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// 输出样式：plain / color / table
    #[arg(long, value_enum, value_name = "STYLE")]
    pub style: Option<OutputStyle>,

    /// 颜色开关：auto / always / never
    #[arg(long, value_enum, value_name = "WHEN")]
    pub color: Option<ColorMode>,

    /// 日志级别：error / warn / info / debug / trace
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// 指定配置文件路径
    #[arg(short = 'C', long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// 打印一份示例配置到标准输出并退出
    #[arg(long)]
    pub print_config: bool,
}
