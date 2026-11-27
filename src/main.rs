//! rust-ping —— 跨平台命令行 ICMP ping 工具。

mod cli;
mod config;
mod icmp;
mod output;
mod pinger;
mod stats;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use clap::Parser;

use crate::cli::Cli;
use crate::config::{example_config, find_default_config, load, merge, Options};

fn init_logging(opts: &Options) {
    let level = if opts.verbose && opts.log_level != "debug" && opts.log_level != "trace" {
        "debug"
    } else {
        opts.log_level.as_str()
    };
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(level);
    builder.format_timestamp(None);
    let _ = builder.try_init();
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config {
        print!("{}", example_config());
        return Ok(());
    }

    let file_cfg = match &cli.config {
        Some(path) => {
            if !path.is_file() {
                bail!("配置文件不存在: {}", path.display());
            }
            Some(load(path)?)
        }
        None => match find_default_config() {
            Some(path) => Some(load(&path)?),
            None => None,
        },
    };

    let opts = merge(&cli, file_cfg.as_ref())?;
    output::setup_color(opts.color, opts.style);
    init_logging(&opts);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = stop.clone();
    ctrlc::set_handler(move || {
        stop_handler.store(true, Ordering::SeqCst);
    })?;

    let target = pinger::resolve(&opts.target)?;
    log::debug!("目标 {} 解析为 {}", opts.target, target);
    log::debug!("生效配置: {:?}", opts);

    pinger::Pinger::new(opts, target, stop)?.run()?;
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
