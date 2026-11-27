//! 输出样式：普通文本、颜色高亮、表格。

use std::sync::Mutex;
use std::time::Duration;

use clap::ValueEnum;
use colored::Colorize;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use serde::Deserialize;

use crate::stats::Stats;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputStyle {
    Plain,
    Color,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

/// 根据颜色开关与样式决定是否启用 ANSI 颜色。
pub fn setup_color(mode: ColorMode, style: OutputStyle) {
    match mode {
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
        ColorMode::Auto => {
            if style == OutputStyle::Color {
                colored::control::set_override(true);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplyInfo {
    pub seq: u32,
    pub ttl: u8,
    pub bytes: usize,
    pub rtt: Duration,
    pub from: String,
}

pub trait Reporter {
    fn start(&self, target: &str, resolved: &str, payload: usize);
    fn reply(&self, info: &ReplyInfo);
    fn timeout(&self, seq: u32);
    fn warning(&self, msg: &str);
    /// 与某个序号绑定的失败。默认退化为普通告警，表格样式会将其记录为一行。
    fn error(&self, seq: u32, msg: &str) {
        let _ = seq;
        self.warning(msg);
    }
    fn summary(&self, stats: &Stats, target: &str);
}

fn rtt_line(stats: &Stats) -> String {
    match (stats.min(), stats.avg(), stats.max()) {
        (Some(a), Some(b), Some(c)) => format!(
            "rtt min/avg/max = {:.3}/{:.3}/{:.3} ms",
            Stats::ms(a),
            Stats::ms(b),
            Stats::ms(c)
        ),
        _ => "rtt min/avg/max = n/a".to_string(),
    }
}

fn summary_lines(stats: &Stats, target: &str) -> Vec<String> {
    vec![
        format!("--- {} ping statistics ---", target),
        format!(
            "{} packets transmitted, {} received, {:.1}% packet loss, time {:.0}ms",
            stats.sent,
            stats.received,
            stats.loss(),
            Stats::ms(stats.elapsed())
        ),
        rtt_line(stats),
    ]
}

struct PlainReporter;

impl Reporter for PlainReporter {
    fn start(&self, target: &str, resolved: &str, payload: usize) {
        println!("PING {} ({}): {} data bytes", target, resolved, payload);
    }

    fn reply(&self, info: &ReplyInfo) {
        println!(
            "{} bytes from {}: icmp_seq={} ttl={} time={:.3} ms",
            info.bytes,
            info.from,
            info.seq,
            info.ttl,
            Stats::ms(info.rtt)
        );
    }

    fn timeout(&self, seq: u32) {
        println!("Request timeout for icmp_seq {}", seq);
    }

    fn warning(&self, msg: &str) {
        eprintln!("warning: {}", msg);
    }

    fn summary(&self, stats: &Stats, target: &str) {
        for line in summary_lines(stats, target) {
            println!("{}", line);
        }
    }
}

struct ColorReporter;

impl Reporter for ColorReporter {
    fn start(&self, target: &str, resolved: &str, payload: usize) {
        println!(
            "{} {} ({}): {} data bytes",
            "PING".bold().cyan(),
            target.bold(),
            resolved,
            payload
        );
    }

    fn reply(&self, info: &ReplyInfo) {
        println!(
            "{} bytes from {}: icmp_seq={} ttl={} time={} ms",
            info.bytes.to_string().bold(),
            info.from.bold().green(),
            format!("{}", info.seq).cyan(),
            info.ttl.to_string().magenta(),
            format!("{:.3}", Stats::ms(info.rtt)).yellow().bold()
        );
    }

    fn timeout(&self, seq: u32) {
        println!("{} for icmp_seq {}", "Request timeout".red().bold(), seq);
    }

    fn warning(&self, msg: &str) {
        eprintln!("{} {}", "warning:".yellow().bold(), msg);
    }

    fn summary(&self, stats: &Stats, target: &str) {
        println!("{}", format!("--- {} ping statistics ---", target).bold());
        let loss = format!("{:.1}% packet loss", stats.loss());
        let loss_colored = if stats.loss() > 0.0 {
            loss.red().bold()
        } else {
            loss.green().bold()
        };
        println!(
            "{} packets transmitted, {} received, {}, time {:.0}ms",
            stats.sent,
            stats.received.to_string().green(),
            loss_colored,
            Stats::ms(stats.elapsed())
        );
        println!("{}", rtt_line(stats).cyan());
    }
}

/// 表格的一行：回复、超时或发送/接收失败。
#[derive(Debug, Clone)]
enum TableRow {
    Reply(ReplyInfo),
    Timeout(u32),
    Error(u32),
}

/// 表格样式不在 ping 过程中逐包打印，而是缓存全部结果，
/// 在 summary() 时一次性输出「明细表 + 汇总表」。
struct TableReporter {
    header: Mutex<Option<(String, String, usize)>>,
    rows: Mutex<Vec<TableRow>>,
    warnings: Mutex<Vec<String>>,
}

impl TableReporter {
    fn new() -> Self {
        Self {
            header: Mutex::new(None),
            rows: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
        }
    }

    fn rows(&self) -> std::sync::MutexGuard<'_, Vec<TableRow>> {
        self.rows.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Reporter for TableReporter {
    fn start(&self, target: &str, resolved: &str, payload: usize) {
        *self.header.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((target.to_string(), resolved.to_string(), payload));
    }

    fn reply(&self, info: &ReplyInfo) {
        self.rows().push(TableRow::Reply(info.clone()));
    }

    fn timeout(&self, seq: u32) {
        self.rows().push(TableRow::Timeout(seq));
    }

    fn warning(&self, msg: &str) {
        self.warnings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(msg.to_string());
    }

    fn error(&self, seq: u32, msg: &str) {
        self.warning(msg);
        self.rows().push(TableRow::Error(seq));
    }

    fn summary(&self, stats: &Stats, target: &str) {
        // 告警统一在表格前输出，ping 过程中表格样式保持静默。
        for msg in self
            .warnings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            eprintln!("warning: {}", msg);
        }

        if let Some((ping_target, resolved, payload)) =
            self.header.lock().unwrap_or_else(|e| e.into_inner()).take()
        {
            println!(
                "PING {} ({}) — 载荷 {} 字节，ICMP Echo",
                ping_target, resolved, payload
            );
        }

        let mut rows = self.rows();
        rows.sort_by_key(|row| match row {
            TableRow::Reply(info) => info.seq,
            TableRow::Timeout(seq) | TableRow::Error(seq) => *seq,
        });

        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_content_arrangement(ContentArrangement::Dynamic);
        table.set_header(vec!["Seq", "From", "TTL", "Bytes", "RTT (ms)", "状态"]);
        for row in rows.iter() {
            match row {
                TableRow::Reply(info) => {
                    table.add_row(vec![
                        info.seq.to_string(),
                        info.from.clone(),
                        info.ttl.to_string(),
                        info.bytes.to_string(),
                        format!("{:.3}", Stats::ms(info.rtt)),
                        "reply".to_string(),
                    ]);
                }
                TableRow::Timeout(seq) => {
                    table.add_row(vec![
                        seq.to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "timeout".to_string(),
                    ]);
                }
                TableRow::Error(seq) => {
                    table.add_row(vec![
                        seq.to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "-".to_string(),
                        "error".to_string(),
                    ]);
                }
            }
        }
        println!("{}", table);

        let mut summary_table = Table::new();
        summary_table.load_preset(UTF8_FULL);
        summary_table.set_header(vec!["指标", "数值"]);
        summary_table.add_row(vec!["目标".to_string(), target.to_string()]);
        summary_table.add_row(vec![
            "发送 / 接收".to_string(),
            format!("{} / {}", stats.sent, stats.received),
        ]);
        summary_table.add_row(vec!["丢包率".to_string(), format!("{:.1}%", stats.loss())]);
        summary_table.add_row(vec![
            "最小 / 平均 / 最大 RTT (ms)".to_string(),
            match (stats.min(), stats.avg(), stats.max()) {
                (Some(a), Some(b), Some(c)) => format!(
                    "{:.3} / {:.3} / {:.3}",
                    Stats::ms(a),
                    Stats::ms(b),
                    Stats::ms(c)
                ),
                _ => "n/a".to_string(),
            },
        ]);
        summary_table.add_row(vec![
            "总耗时 (ms)".to_string(),
            format!("{:.0}", Stats::ms(stats.elapsed())),
        ]);
        println!("{}", summary_table);
    }
}

pub fn make_reporter(style: OutputStyle) -> Box<dyn Reporter> {
    match style {
        OutputStyle::Plain => Box::new(PlainReporter),
        OutputStyle::Color => Box::new(ColorReporter),
        OutputStyle::Table => Box::new(TableReporter::new()),
    }
}
