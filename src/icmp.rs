//! ICMP Echo 报文构造与跨平台收发。
//!
//! 策略：
//! 1. 优先使用「DGRAM 型 ICMP socket」——Linux/macOS 上非特权即可收发 ping。
//! 2. 若内核不支持 DGRAM 型 ICMP，回退到 RAW socket（需要 CAP_NET_RAW / 管理员权限），
//!    并在错误中给出各平台的具体提权指引。
//! 3. TTL 通过 IP_RECVTTL / IPV6_RECVHOPLIMIT 控制消息获取；拿不到时回退到
//!    发送时设置的 TTL（Windows 的 cmsg 支持有限）。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use socket2::{Domain, Protocol, Socket, Type};

const ICMPV4_ECHO_REQUEST: u8 = 8;
const ICMPV4_ECHO_REPLY: u8 = 0;
const ICMPV6_ECHO_REQUEST: u8 = 128;
const ICMPV6_ECHO_REPLY: u8 = 129;
const HEADER_LEN: usize = 8;

#[derive(Debug, Clone)]
pub struct IcmpReply {
    pub identifier: u16,
    pub sequence: u16,
    pub ttl: u8,
}

/// 一次接收尝试的结果。区分「收到非回复包」和「超时」，避免把回环到
/// 本地 socket 的 Echo Request 误判为超时而提前结束等待。
#[derive(Debug)]
pub enum RecvOutcome {
    Reply(IcmpReply),
    Ignored,
    Timeout,
}

/// 因特网校验和（RFC 1071）。
// `as_chunks` 直到 Rust 1.88 才稳定，这里保持 1.70+ 兼容性。
#[allow(clippy::chunks_exact_to_as_chunks)]
fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);
    for pair in &mut chunks {
        sum += u16::from_be_bytes([pair[0], pair[1]]) as u32;
    }
    if let Some(&last) = chunks.remainder().first() {
        sum += (last as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

fn echo_request(is_v6: bool, identifier: u16, seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER_LEN + payload.len());
    packet.push(if is_v6 {
        ICMPV6_ECHO_REQUEST
    } else {
        ICMPV4_ECHO_REQUEST
    });
    packet.push(0); // code
    packet.extend_from_slice(&[0, 0]); // checksum 占位
    packet.extend_from_slice(&identifier.to_be_bytes());
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(payload);
    packet
}

/// 构造 ICMPv6 伪首部并计算校验和。
fn icmpv6_checksum(src: &IpAddr, dst: &IpAddr, packet: &[u8]) -> u16 {
    let (src, dst) = match (src, dst) {
        (IpAddr::V6(s), IpAddr::V6(d)) => (s.octets(), d.octets()),
        // 理论上不会发生；退化成用零地址，交由内核覆盖。
        _ => ([0u8; 16], [0u8; 16]),
    };
    let mut pseudo = Vec::with_capacity(40 + packet.len());
    pseudo.extend_from_slice(&src);
    pseudo.extend_from_slice(&dst);
    pseudo.extend_from_slice(&(packet.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0, 58]); // Next Header = ICMPv6
    pseudo.extend_from_slice(packet);
    checksum(&pseudo)
}

fn permission_error(err: io::Error) -> anyhow::Error {
    #[cfg(windows)]
    let hint = "Windows 下发送 ICMP 需要管理员权限，请以管理员身份重新运行";
    #[cfg(not(windows))]
    let hint =
        "需要 CAP_NET_RAW 权限：可执行 sudo setcap cap_net_raw+ep ./rust-ping，或使用 sudo 运行";
    anyhow!("无法创建 ICMP socket: {}。{}", err, hint)
}

pub struct IcmpSocket {
    socket: Socket,
    target: IpAddr,
    /// 目标 socket 地址。用 send_to 而非 connect：在 TUN/代理环境下
    /// connect 后调用 send 会内核报 EPIPE（Broken pipe），系统 ping 也是用 sendto。
    target_addr: SocketAddr,
    source: IpAddr,
    identifier: u16,
    ttl: u8,
    is_v6: bool,
}

impl IcmpSocket {
    pub fn new(target: IpAddr, ttl: u8) -> Result<Self> {
        let is_v6 = target.is_ipv6();
        let (domain, proto) = if is_v6 {
            (Domain::IPV6, Protocol::ICMPV6)
        } else {
            (Domain::IPV4, Protocol::ICMPV4)
        };

        // DGRAM 型 ICMP socket 在 Linux/macOS 上无需 root。
        let socket = match Socket::new(domain, Type::DGRAM, Some(proto)) {
            Ok(s) => {
                log::debug!("使用 DGRAM 型 ICMP socket（非特权模式）");
                s
            }
            Err(dgram_err) => {
                log::debug!(
                    "DGRAM 型 ICMP socket 不可用: {}，回退 RAW socket",
                    dgram_err
                );
                Socket::new(domain, Type::RAW, Some(proto))
                    .map_err(permission_error)
                    .with_context(|| "创建 ICMP socket 失败")?
            }
        };

        let target_addr = SocketAddr::new(target, 0);

        if is_v6 {
            socket
                .set_unicast_hops_v6(ttl as u32)
                .with_context(|| "设置 IPv6 Hop Limit 失败")?;
        } else {
            socket
                .set_ttl(ttl as u32)
                .with_context(|| "设置 IPv4 TTL 失败")?;
        }
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        // 尽力启用 TTL 控制消息；不支持时静默忽略（用发送 TTL 作为回退显示）。
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let fd = socket.as_raw_fd();
            let one: libc::c_int = 1;
            unsafe {
                if is_v6 {
                    libc::setsockopt(
                        fd,
                        libc::IPPROTO_IPV6,
                        libc::IPV6_RECVHOPLIMIT,
                        &one as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );
                } else {
                    libc::setsockopt(
                        fd,
                        libc::IPPROTO_IP,
                        libc::IP_RECVTTL,
                        &one as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );
                }
            }
        }

        let source = socket
            .local_addr()
            .ok()
            .and_then(|a| a.as_socket().map(|s| s.ip()))
            .unwrap_or(if is_v6 {
                IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED)
            } else {
                IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
            });

        Ok(Self {
            socket,
            target,
            target_addr,
            source,
            identifier: (std::process::id() & 0xFFFF) as u16,
            ttl,
            is_v6,
        })
    }

    pub fn identifier(&self) -> u16 {
        self.identifier
    }

    pub fn send(&self, seq: u16, payload: &[u8]) -> Result<()> {
        let mut packet = echo_request(self.is_v6, self.identifier, seq, payload);
        if self.is_v6 {
            let csum = icmpv6_checksum(&self.source, &self.target, &packet);
            packet[2..4].copy_from_slice(&csum.to_be_bytes());
        } else {
            let csum = checksum(&packet);
            packet[2..4].copy_from_slice(&csum.to_be_bytes());
        }
        // 用 send_to 指定目标地址，而不是 connect + send。
        // macOS 上 connect 到经 utun 代理路由的地址后，send 会被内核以 EPIPE 拒绝；
        // 系统 ping 同样使用 sendto。
        self.socket
            .send_to(&packet, &self.target_addr.into())
            .with_context(|| {
                format!(
                    "发送 ICMP Echo 请求失败{}",
                    if cfg!(windows) {
                        "（请以管理员身份运行）"
                    } else {
                        ""
                    }
                )
            })?;
        Ok(())
    }

    /// 接收一个 ICMP Echo Reply。
    pub fn recv(&self, buf: &mut [u8], timeout: Duration) -> Result<RecvOutcome> {
        self.socket.set_read_timeout(Some(timeout))?;
        let (n, ttl) = match recv_once(&self.socket, buf) {
            Ok(v) => v,
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                return Ok(RecvOutcome::Timeout)
            }
            Err(e) => return Err(e).with_context(|| "接收 ICMP 回复失败"),
        };
        // macOS 的 DGRAM 型 ICMP socket 会把完整 IP 头一起返回；Linux 通常不含。
        // 通过版本字段 + 协议字段识别并跳过 IP 头。
        let mut offset = 0usize;
        let mut header_ttl = None;
        if n >= 20 && buf[0] >> 4 == 4 && buf[9] == 1 {
            offset = (buf[0] & 0x0F) as usize * 4;
            header_ttl = Some(buf[8]);
            log::debug!("收到带 IPv4 头的报文，头长 {} 字节，TTL {}", offset, buf[8]);
        } else if n >= 40 && buf[0] >> 4 == 6 && buf[6] == 58 {
            offset = 40;
            header_ttl = Some(buf[7]);
            log::debug!("收到带 IPv6 头的报文，Hop Limit {}", buf[7]);
        }
        if n < offset + HEADER_LEN {
            return Ok(RecvOutcome::Ignored);
        }
        let expected_type = if self.is_v6 {
            ICMPV6_ECHO_REPLY
        } else {
            ICMPV4_ECHO_REPLY
        };
        if buf[offset] != expected_type {
            // 忽略自己回环的 Echo Request、ICMP 错误/重定向等非 Echo Reply 报文。
            log::debug!("忽略非 Echo Reply 报文: type=0x{:02X}", buf[offset]);
            return Ok(RecvOutcome::Ignored);
        }
        let identifier = u16::from_be_bytes([buf[offset + 4], buf[offset + 5]]);
        let sequence = u16::from_be_bytes([buf[offset + 6], buf[offset + 7]]);
        Ok(RecvOutcome::Reply(IcmpReply {
            identifier,
            sequence,
            ttl: ttl.or(header_ttl).unwrap_or(self.ttl),
        }))
    }
}

#[cfg(unix)]
fn recv_once(socket: &Socket, buf: &mut [u8]) -> io::Result<(usize, Option<u8>)> {
    use std::os::fd::AsRawFd;

    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let mut control = [0u8; 256];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_name = &mut storage as *mut _ as *mut libc::c_void;
    msg.msg_namelen = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr() as *mut libc::c_void;
    // Linux 上 msg_controllen 是 usize，macOS/BSD 上是 socklen_t(u32)，
    // 用 `as _` 让编译器按目标平台推断，保证跨平台编译。
    msg.msg_controllen = control.len() as _;

    let n = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, 0) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut ttl = None;
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            let header = &*cmsg;
            let is_ttl = (header.cmsg_level == libc::IPPROTO_IP
                && header.cmsg_type == libc::IP_TTL)
                || (header.cmsg_level == libc::IPPROTO_IPV6
                    && header.cmsg_type == libc::IPV6_HOPLIMIT);
            if is_ttl {
                let data = libc::CMSG_DATA(cmsg) as *const u8;
                let len = header.cmsg_len as usize - libc::CMSG_LEN(0) as usize;
                // Linux 上是 u8；macOS 上是 int。按长度区分。
                ttl = Some(if len >= 4 {
                    *(data as *const u32) as u8
                } else {
                    *data
                });
                break;
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    Ok((n as usize, ttl))
}

#[cfg(windows)]
fn recv_once(socket: &Socket, buf: &mut [u8]) -> io::Result<(usize, Option<u8>)> {
    use std::io::Read;
    // Windows 的 WSARecvMsg 控制消息 API 复杂且行为因版本而异；
    // 这里用标准 read，TTL 由 IcmpSocket::recv 回退到发送 TTL。
    let n = (&*socket).read(buf)?;
    Ok((n, None))
}
