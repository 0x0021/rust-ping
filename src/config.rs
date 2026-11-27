//! 配置加载与合并：内置默认值 < 配置文件 < 命令行参数。

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::cli::Cli;
use crate::output::{ColorMode, OutputStyle};

/// 最终生效的配置，所有字段均有确定值。
#[derive(Debug, Clone)]
pub struct Options {
    pub target: String,
    /// None 表示持续运行
    pub count: Option<u32>,
    pub interval: f64,
    pub timeout: f64,
    pub payload_size: usize,
    pub ttl: u8,
    pub log_level: String,
    pub style: OutputStyle,
    pub color: ColorMode,
    pub verbose: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            target: String::new(),
            count: Some(4),
            interval: 1.0,
            timeout: 1.0,
            payload_size: 56,
            ttl: 64,
            log_level: "warn".to_string(),
            style: OutputStyle::Plain,
            color: ColorMode::Auto,
            verbose: false,
        }
    }
}

/// 配置文件中的字段。全部可选，未出现的字段沿用默认值。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    pub target: Option<String>,
    pub count: Option<u32>,
    pub interval: Option<f64>,
    pub timeout: Option<f64>,
    pub payload_size: Option<usize>,
    pub ttl: Option<u8>,
    pub log_level: Option<String>,
    pub style: Option<OutputStyle>,
    pub color: Option<ColorMode>,
}

/// 按优先级查找默认配置文件：当前目录 > 用户配置目录。
pub fn find_default_config() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("rust-ping.toml")];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".config/rust-ping/config.toml"));
    }
    candidates.into_iter().find(|p| p.is_file() && p.exists())
}

/// 读取并解析 TOML 配置文件。
pub fn load(path: &Path) -> Result<FileConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
    toml::from_str(&text)
        .with_context(|| format!("解析配置文件失败（请检查 TOML 语法）: {}", path.display()))
}

/// 生成一份带注释的示例配置文本。
pub fn example_config() -> &'static str {
    include_str!("../config.example.toml")
}

fn normalize_count(n: u32) -> Option<u32> {
    if n == 0 {
        None
    } else {
        Some(n)
    }
}

/// 按 配置文件 -> 命令行 的顺序覆盖默认值。
pub fn merge(cli: &Cli, file: Option<&FileConfig>) -> Result<Options> {
    let mut opts = Options::default();

    if let Some(f) = file {
        if let Some(v) = &f.target {
            opts.target = v.clone();
        }
        if let Some(v) = f.count {
            opts.count = normalize_count(v);
        }
        if let Some(v) = f.interval {
            opts.interval = v;
        }
        if let Some(v) = f.timeout {
            opts.timeout = v;
        }
        if let Some(v) = f.payload_size {
            opts.payload_size = v;
        }
        if let Some(v) = f.ttl {
            opts.ttl = v;
        }
        if let Some(v) = &f.log_level {
            opts.log_level = v.clone();
        }
        if let Some(v) = f.style {
            opts.style = v;
        }
        if let Some(v) = f.color {
            opts.color = v;
        }
    }

    if let Some(v) = &cli.target {
        opts.target = v.clone();
    }
    if let Some(v) = cli.count {
        opts.count = normalize_count(v);
    }
    if cli.continuous {
        opts.count = None;
    }
    if let Some(v) = cli.interval {
        opts.interval = v;
    }
    if let Some(v) = cli.timeout {
        opts.timeout = v;
    }
    if let Some(v) = cli.payload_size {
        opts.payload_size = v;
    }
    if let Some(v) = cli.ttl {
        opts.ttl = v;
    }
    if let Some(v) = &cli.log_level {
        opts.log_level = v.clone();
    }
    if let Some(v) = cli.style {
        opts.style = v;
    }
    if let Some(v) = cli.color {
        opts.color = v;
    }
    opts.verbose = cli.verbose;

    if opts.target.trim().is_empty() {
        bail!("必须指定目标：命令行传入 TARGET，或在配置文件中设置 target");
    }
    if opts.interval <= 0.0 {
        bail!("interval 必须大于 0 秒");
    }
    if opts.timeout <= 0.0 {
        bail!("timeout 必须大于 0 秒");
    }
    if opts.payload_size > 65_500 {
        bail!("payload_size 不能超过 65500 字节");
    }

    Ok(opts)
}
