//! ping 主循环：发送、接收匹配、统计、优雅退出。

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};

use crate::config::Options;
use crate::icmp::IcmpSocket;
use crate::output::{ReplyInfo, Reporter};
use crate::stats::Stats;

/// 解析域名或 IP；域名优先返回 IPv4 地址，其次 IPv6。
pub fn resolve(target: &str) -> Result<SocketAddr> {
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, 0));
    }
    let addrs: Vec<SocketAddr> = (target, 0)
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("解析域名 '{}' 失败: {}", target, e))?
        .collect();
    if let Some(v4) = addrs.iter().find(|a| a.is_ipv4()) {
        return Ok(*v4);
    }
    match addrs.into_iter().next() {
        Some(a) => Ok(a),
        None => bail!("无法解析目标 '{}'：未找到可用的 IP 地址", target),
    }
}

fn build_payload(size: usize) -> Vec<u8> {
    let mut payload = vec![0xA5u8; size];
    if size >= 8 {
        // 前 8 字节写入纳秒时间戳，让载荷内容真实且可区分。
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        payload[..8].copy_from_slice(&ts.to_be_bytes());
    }
    payload
}

pub struct Pinger {
    socket: IcmpSocket,
    target: SocketAddr,
    opts: Options,
    reporter: Box<dyn Reporter>,
    stop: Arc<AtomicBool>,
}

impl Pinger {
    pub fn new(opts: Options, target: SocketAddr, stop: Arc<AtomicBool>) -> Result<Self> {
        let socket = IcmpSocket::new(target.ip(), opts.ttl)?;
        let reporter = crate::output::make_reporter(opts.style);
        Ok(Self {
            socket,
            target,
            opts,
            reporter,
            stop,
        })
    }

    pub fn run(&mut self) -> Result<Stats> {
        let count = self.opts.count.unwrap_or(u32::MAX);
        let timeout = Duration::from_secs_f64(self.opts.timeout);
        let interval = Duration::from_secs_f64(self.opts.interval);
        let payload = build_payload(self.opts.payload_size);
        let mut stats = Stats::default();
        stats.begin();

        self.reporter.start(
            &self.opts.target,
            &self.target.ip().to_string(),
            self.opts.payload_size,
        );
        log::debug!(
            "开始 ping: 目标={} 次数={} 间隔={:?} 超时={:?} 载荷={}",
            self.target,
            self.opts.count.map(|n| n.to_string()).unwrap_or_else(|| "持续".into()),
            interval,
            timeout,
            self.opts.payload_size
        );

        let mut seq: u32 = 1;
        while seq <= count && !self.stop.load(Ordering::SeqCst) {
            let sent_at = Instant::now();
            match self.socket.send(seq as u16, &payload) {
                Ok(()) => stats.record_sent(),
                Err(e) => {
                    self.reporter.warning(&format!("第 {} 包发送失败: {:#}", seq, e));
                    stats.record_error();
                    stats.record_sent();
                    seq += 1;
                    thread::sleep(interval);
                    continue;
                }
            }

            let mut got_reply = false;
            let deadline = sent_at + timeout;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() || self.stop.load(Ordering::SeqCst) {
                    break;
                }
                let mut buf = [0u8; 65_536];
                match self.socket.recv(&mut buf, remaining) {
                    Ok(crate::icmp::RecvOutcome::Reply(reply)) => {
                        // 主要按序号匹配。部分平台（尤其 macOS 的 DGRAM ICMP）会改写
                        // identifier，因此只在序号匹配时接受，identifier 作日志参考。
                        if reply.sequence != seq as u16 {
                            // 延迟到达的旧包或他人流量，继续等待本次回复。
                            continue;
                        }
                        if reply.identifier != self.socket.identifier() {
                            log::debug!(
                                "identifier 与预期不同（收到 {}，预期 {}），内核可能已改写",
                                reply.identifier,
                                self.socket.identifier()
                            );
                        }
                        let rtt = sent_at.elapsed();
                        self.reporter.reply(&ReplyInfo {
                            seq,
                            ttl: reply.ttl,
                            bytes: self.opts.payload_size + 8,
                            rtt,
                            from: self.target.ip().to_string(),
                        });
                        stats.record_reply(rtt);
                        got_reply = true;
                        break;
                    }
                    Ok(crate::icmp::RecvOutcome::Ignored) => continue, // 非回复包，继续等
                    Ok(crate::icmp::RecvOutcome::Timeout) => break,    // 超时
                    Err(e) => {
                        self.reporter.warning(&format!("接收回复失败: {:#}", e));
                        stats.record_error();
                        break;
                    }
                }
            }

            if !got_reply && !self.stop.load(Ordering::SeqCst) {
                self.reporter.timeout(seq);
            }

            seq += 1;
            if seq <= count && !self.stop.load(Ordering::SeqCst) {
                thread::sleep(interval);
            }
        }

        stats.finish();
        self.reporter.summary(&stats, &self.opts.target);
        Ok(stats)
    }
}
