//! 往返延迟统计。

use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Stats {
    pub sent: u64,
    pub received: u64,
    pub rtts: Vec<Duration>,
    pub errors: u64,
    start: Option<Instant>,
    end: Option<Instant>,
}

impl Stats {
    pub fn begin(&mut self) {
        self.start = Some(Instant::now());
    }

    pub fn finish(&mut self) {
        self.end = Some(Instant::now());
    }

    pub fn record_sent(&mut self) {
        self.sent += 1;
    }

    pub fn record_reply(&mut self, rtt: Duration) {
        self.received += 1;
        self.rtts.push(rtt);
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    pub fn loss(&self) -> f64 {
        if self.sent == 0 {
            return 0.0;
        }
        (self.sent.saturating_sub(self.received)) as f64 / self.sent as f64 * 100.0
    }

    pub fn min(&self) -> Option<Duration> {
        self.rtts.iter().min().copied()
    }

    pub fn avg(&self) -> Option<Duration> {
        if self.rtts.is_empty() {
            return None;
        }
        let total: Duration = self.rtts.iter().sum();
        Some(total / self.rtts.len() as u32)
    }

    pub fn max(&self) -> Option<Duration> {
        self.rtts.iter().max().copied()
    }

    pub fn elapsed(&self) -> Duration {
        match (self.start, self.end) {
            (Some(a), Some(b)) => b.saturating_duration_since(a),
            (Some(a), None) => a.elapsed(),
            _ => Duration::ZERO,
        }
    }

    pub fn ms(d: Duration) -> f64 {
        d.as_secs_f64() * 1000.0
    }
}
