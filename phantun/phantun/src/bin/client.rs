use clap::{crate_version, Arg, ArgAction, Command};
use fake_tcp::packet::MAX_PACKET_LEN;
use fake_tcp::{Socket, Stack};
use log::{debug, error, info};
use phantun::utils::{assign_ipv6_address, new_udp_reuseport, udp_recv_pktinfo};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use tokio::time;
use tokio_tun::TunBuilder;
use tokio_util::sync::CancellationToken;

use phantun::UDP_TTL;

// Keepalive configuration
const KEEPALIVE_INTERVAL: time::Duration = time::Duration::from_secs(30);
const KEEPALIVE_PACKET: &[u8] = &[0u8; 1]; // 1-byte keepalive packet

// Connection info with health status and keepalive tracking
struct SocketInfo {
    socket: Arc<Socket>,
    id: usize,
    is_healthy: Arc<AtomicBool>,
    last_activity: Arc<RwLock<time::Instant>>,
}

// Connection pool for load balancing with health monitoring
struct ConnectionPool {
    sockets: Arc<RwLock<Vec<SocketInfo>>>,
    next_idx: AtomicUsize,
    target_count: usize,
    #[allow(dead_code)] // Reserved for future auto-reconnection feature
    remote_addr: SocketAddr,
}

impl ConnectionPool {
    fn new(sockets: Vec<Arc<Socket>>, target_count: usize, remote_addr: SocketAddr) -> Self {
        let now = time::Instant::now();
        let socket_infos: Vec<SocketInfo> = sockets
            .into_iter()
            .enumerate()
            .map(|(id, socket)| SocketInfo {
                socket,
                id,
                is_healthy: Arc::new(AtomicBool::new(true)),
                last_activity: Arc::new(RwLock::new(now)),
            })
            .collect();

        Self {
            sockets: Arc::new(RwLock::new(socket_infos)),
            next_idx: AtomicUsize::new(0),
            target_count,
            remote_addr,
        }
    }

    async fn get_next(&self) -> Option<(Arc<Socket>, usize, Arc<AtomicBool>, Arc<RwLock<time::Instant>>)> {
        let sockets = self.sockets.read().await;

        if sockets.is_empty() {
            return None;
        }

        // Try to find a healthy connection (max attempts = socket count)
        for _ in 0..sockets.len() {
            let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) % sockets.len();
            let info = &sockets[idx];

            if info.is_healthy.load(Ordering::Relaxed) {
                return Some((
                    info.socket.clone(),
                    info.id,
                    info.is_healthy.clone(),
                    info.last_activity.clone(),
                ));
            }
        }

        // No healthy connection found, return first one as fallback
        let info = &sockets[0];
        Some((
            info.socket.clone(),
            info.id,
            info.is_healthy.clone(),
            info.last_activity.clone(),
        ))
    }

    #[allow(dead_code)] // Reserved for API completeness
    async fn update_activity(&self, id: usize) {
        let sockets = self.sockets.read().await;
        if let Some(info) = sockets.iter().find(|s| s.id == id) {
            *info.last_activity.write().await = time::Instant::now();
        }
    }

    #[allow(dead_code)] // Reserved for future fine-grained health management
    async fn mark_unhealthy(&self, id: usize) {
        let sockets = self.sockets.read().await;
        if let Some(info) = sockets.iter().find(|s| s.id == id) {
            info.is_healthy.store(false, Ordering::Relaxed);
            info!("Marked TCP connection {} as unhealthy", id);
        }
    }

    async fn remove_unhealthy(&self) -> usize {
        let mut sockets = self.sockets.write().await;
        let before_count = sockets.len();
        sockets.retain(|info| info.is_healthy.load(Ordering::Relaxed));
        let removed = before_count - sockets.len();

        if removed > 0 {
            info!("Removed {} unhealthy connections, {} remaining", removed, sockets.len());
        }

        sockets.len()
    }

    #[allow(dead_code)] // Reserved for future auto-reconnection feature
    async fn add_connection(&self, socket: Arc<Socket>) {
        let mut sockets = self.sockets.write().await;
        let id = sockets.iter().map(|s| s.id).max().unwrap_or(0) + 1;

        sockets.push(SocketInfo {
            socket,
            id,
            is_healthy: Arc::new(AtomicBool::new(true)),
            last_activity: Arc::new(RwLock::new(time::Instant::now())),
        });

        info!("Added new TCP connection {}, total: {}", id, sockets.len());
    }

    async fn get_all_sockets(&self) -> Vec<(Arc<Socket>, usize, Arc<AtomicBool>, Arc<RwLock<time::Instant>>)> {
        let sockets = self.sockets.read().await;
        sockets
            .iter()
            .map(|info| (
                info.socket.clone(),
                info.id,
                info.is_healthy.clone(),
                info.last_activity.clone(),
            ))
            .collect()
    }

    async fn healthy_count(&self) -> usize {
        let sockets = self.sockets.read().await;
        sockets.iter().filter(|s| s.is_healthy.load(Ordering::Relaxed)).count()
    }

    fn target_count(&self) -> usize {
        self.target_count
    }

    #[allow(dead_code)] // Reserved for future auto-reconnection feature
    fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    pretty_env_logger::init();

    let matches = Command::new("Phantun Client")
        .version(crate_version!())
        .author("Datong Sun (github.com/dndx)")
        .arg(
            Arg::new("local")
                .short('l')
                .long("local")
                .required(true)
                .value_name("IP:PORT")
                .help("Sets the IP and port where Phantun Client listens for incoming UDP datagrams, IPv6 address need to be specified as: \"[IPv6]:PORT\"")
        )
        .arg(
            Arg::new("remote")
                .short('r')
                .long("remote")
                .required(true)
                .value_name("IP or HOST NAME:PORT")
                .help("Sets the address or host name and port where Phantun Client connects to Phantun Server, IPv6 address need to be specified as: \"[IPv6]:PORT\"")
        )
        .arg(
            Arg::new("tun")
                .long("tun")
                .required(false)
                .value_name("tunX")
                .help("Sets the Tun interface name, if absent, pick the next available name")
                .default_value("")
        )
        .arg(
            Arg::new("tun_local")
                .long("tun-local")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv4 local address (O/S's end)")
                .default_value("192.168.200.1")
        )
        .arg(
            Arg::new("tun_peer")
                .long("tun-peer")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv4 destination (peer) address (Phantun Client's end). \
                       You will need to setup SNAT/MASQUERADE rules on your Internet facing interface \
                       in order for Phantun Client to connect to Phantun Server")
                .default_value("192.168.200.2")
        )
        .arg(
            Arg::new("ipv4_only")
                .long("ipv4-only")
                .short('4')
                .required(false)
                .help("Only use IPv4 address when connecting to remote")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["tun_local6", "tun_peer6"]),
        )
        .arg(
            Arg::new("tun_local6")
                .long("tun-local6")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv6 local address (O/S's end)")
                .default_value("fcc8::1")
        )
        .arg(
            Arg::new("tun_peer6")
                .long("tun-peer6")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv6 destination (peer) address (Phantun Client's end). \
                       You will need to setup SNAT/MASQUERADE rules on your Internet facing interface \
                       in order for Phantun Client to connect to Phantun Server")
                .default_value("fcc8::2")
        )
        .arg(
            Arg::new("handshake_packet")
                .long("handshake-packet")
                .required(false)
                .value_name("PATH")
                .help("Specify a file, which, after TCP handshake, its content will be sent as the \
                      first data packet to the server.\n\
                      Note: ensure this file's size does not exceed the MTU of the outgoing interface. \
                      The content is always sent out in a single packet and will not be further segmented")
        )
        .arg(
            Arg::new("num_tcp_conns")
                .long("num-tcp-conns")
                .required(false)
                .value_name("NUM")
                .help("Number of TCP connections to use for load balancing a single UDP stream. \
                      Default is 1 (no load balancing). Set to 2-8 for load balancing.")
                .default_value("1")
        )
        .get_matches();

    let local_addr: SocketAddr = matches
        .get_one::<String>("local")
        .unwrap()
        .parse()
        .expect("bad local address");

    let ipv4_only = matches.get_flag("ipv4_only");

    let remote_addr = tokio::net::lookup_host(matches.get_one::<String>("remote").unwrap())
        .await
        .expect("bad remote address or host")
        .find(|addr| !ipv4_only || addr.is_ipv4())
        .expect("unable to resolve remote host name");
    info!("Remote address is: {}", remote_addr);

    let tun_local: Ipv4Addr = matches
        .get_one::<String>("tun_local")
        .unwrap()
        .parse()
        .expect("bad local address for Tun interface");
    let tun_peer: Ipv4Addr = matches
        .get_one::<String>("tun_peer")
        .unwrap()
        .parse()
        .expect("bad peer address for Tun interface");

    let (tun_local6, tun_peer6) = if matches.get_flag("ipv4_only") {
        (None, None)
    } else {
        (
            matches
                .get_one::<String>("tun_local6")
                .map(|v| v.parse().expect("bad local address for Tun interface")),
            matches
                .get_one::<String>("tun_peer6")
                .map(|v| v.parse().expect("bad peer address for Tun interface")),
        )
    };

    let tun_name = matches.get_one::<String>("tun").unwrap();
    let handshake_packet: Option<Vec<u8>> = matches
        .get_one::<String>("handshake_packet")
        .map(fs::read)
        .transpose()?;
    let num_tcp_conns: usize = matches
        .get_one::<String>("num_tcp_conns")
        .unwrap()
        .parse()
        .expect("invalid number of TCP connections");

    if num_tcp_conns < 1 || num_tcp_conns > 16 {
        panic!("num-tcp-conns must be between 1 and 16");
    }

    if num_tcp_conns > 1 {
        info!("Load balancing enabled with {} TCP connections per UDP stream", num_tcp_conns);
    }

    let num_cpus = num_cpus::get();
    info!("{} cores available", num_cpus);

    let tun = TunBuilder::new()
        .name(tun_name) // if name is empty, then it is set by kernel.
        .up() // or set it up manually using `sudo ip link set <tun-name> up`.
        .address(tun_local)
        .destination(tun_peer)
        .queues(num_cpus)
        .build()
        .unwrap();

    if remote_addr.is_ipv6() {
        assign_ipv6_address(tun[0].name(), tun_local6.unwrap(), tun_peer6.unwrap());
    }

    info!("Created TUN device {}", tun[0].name());

    let udp_sock = Arc::new(new_udp_reuseport(local_addr));
    let connections = Arc::new(RwLock::new(HashMap::<SocketAddr, Arc<ConnectionPool>>::new()));

    let mut stack = Stack::new(tun, tun_peer, tun_peer6);

    let main_loop = tokio::spawn(async move {
        let mut buf_r = [0u8; MAX_PACKET_LEN];

        loop {
            let (size, udp_remote_addr, udp_local_addr) = udp_recv_pktinfo(&udp_sock, &mut buf_r).await?;
            // seen UDP packet to listening socket, this means:
            // 1. It is a new UDP connection, or
            // 2. It is some extra packets not filtered by more specific
            //    connected UDP socket yet
            if let Some(pool) = connections.read().await.get(&udp_remote_addr) {
                // Use round-robin to select next healthy TCP connection
                if let Some((sock, conn_id, health, last_activity)) = pool.get_next().await {
                    if sock.send(&buf_r[..size]).await.is_some() {
                        // Update activity time on successful send
                        *last_activity.write().await = time::Instant::now();
                    } else {
                        // Mark connection as unhealthy
                        health.store(false, Ordering::Relaxed);
                        debug!("Failed to send via connection {}, marked unhealthy", conn_id);
                    }
                }
                continue;
            }

            info!("New UDP client from {}, creating {} TCP connections", udp_remote_addr, num_tcp_conns);

            // Create multiple TCP connections for load balancing
            let mut sockets = Vec::new();
            for i in 0..num_tcp_conns {
                let sock = stack.connect(remote_addr).await;
                if sock.is_none() {
                    error!("Unable to connect to remote {} for connection {}/{}", remote_addr, i+1, num_tcp_conns);
                    continue;
                }

                let sock = Arc::new(sock.unwrap());
                if let Some(ref p) = handshake_packet {
                    if sock.send(p).await.is_none() {
                        error!("Failed to send handshake packet to remote on connection {}/{}, closing connection.", i+1, num_tcp_conns);
                        continue;
                    }

                    debug!("Sent handshake packet to: {} (connection {}/{})", sock, i+1, num_tcp_conns);
                }

                sockets.push(sock);
                info!("Established TCP connection {}/{} for UDP client {}", i+1, num_tcp_conns, udp_remote_addr);
            }

            if sockets.is_empty() {
                error!("Failed to establish any TCP connections for UDP client {}", udp_remote_addr);
                continue;
            }

            // Send first packet using round-robin
            let pool = Arc::new(ConnectionPool::new(sockets, num_tcp_conns, remote_addr));
            if let Some((sock, conn_id, health, last_activity)) = pool.get_next().await {
                if sock.send(&buf_r[..size]).await.is_some() {
                    *last_activity.write().await = time::Instant::now();
                } else {
                    health.store(false, Ordering::Relaxed);
                    debug!("Failed to send first packet via connection {}", conn_id);
                }
            } else {
                error!("No connections available in pool");
                continue;
            }

            assert!(connections
                .write()
                .await
                .insert(udp_remote_addr, pool.clone())
                .is_none());
            debug!("inserted {} fake TCP sockets into connection table", num_tcp_conns);

            // spawn "fastpath" UDP socket and task, this will offload main task
            // from forwarding UDP packets

            let packet_received = Arc::new(Notify::new());
            let quit = CancellationToken::new();

            // For each TCP connection, spawn workers
            let all_sockets = pool.get_all_sockets().await;
            for (sock, tcp_id, health, last_activity) in all_sockets {
                for i in 0..num_cpus {
                    let sock = sock.clone();
                    let pool = pool.clone();
                    let quit = quit.clone();
                    let packet_received = packet_received.clone();
                    let health = health.clone();
                    let last_activity = last_activity.clone();

                    tokio::spawn(async move {
                        let mut buf_udp = [0u8; MAX_PACKET_LEN];
                        let mut buf_tcp = [0u8; MAX_PACKET_LEN];
                        // Always reply from the same address that the peer used to communicate with
                        // us. This avoids a frequent problem with IPv6 privacy extensions when we
                        // erroneously bind to wrong short-lived temporary address even if the peer
                        // explicitly used a persistent address to communicate to us.
                        //
                        // To do so, first bind to (<incoming packet dst_ip>, <local addr port>), and then
                        // connect to (<incoming packet src_ip>, <incoming packet src_port>).
                        let bind_addr = match (udp_remote_addr, udp_local_addr) {
                            (SocketAddr::V4(_), IpAddr::V4(udp_local_ipv4)) => {
                                SocketAddr::V4(SocketAddrV4::new(
                                    udp_local_ipv4,
                                    local_addr.port(),
                                ))
                            }
                            (SocketAddr::V6(udp_remote_addr), IpAddr::V6(udp_local_ipv6)) => {
                                SocketAddr::V6(SocketAddrV6::new(
                                    udp_local_ipv6,
                                    local_addr.port(),
                                    udp_remote_addr.flowinfo(),
                                    udp_remote_addr.scope_id(),
                                ))
                            }
                            (_, _) => {
                                panic!("unexpected family combination for udp_remote_addr={udp_remote_addr} and udp_local_addr={udp_local_addr}");
                            }
                        };
                        let udp_sock = new_udp_reuseport(bind_addr);
                        udp_sock.connect(udp_remote_addr).await.unwrap();

                        loop {
                            tokio::select! {
                                Ok(size) = udp_sock.recv(&mut buf_udp) => {
                                    // Use round-robin to select healthy TCP connection for sending
                                    if let Some((send_sock, send_id, send_health, send_activity)) = pool.get_next().await {
                                        if send_sock.send(&buf_udp[..size]).await.is_some() {
                                            // Update activity on successful send
                                            *send_activity.write().await = time::Instant::now();
                                        } else {
                                            send_health.store(false, Ordering::Relaxed);
                                            debug!("Failed to send via connection {}, marked unhealthy", send_id);
                                            // Continue trying other connections instead of quitting
                                        }
                                    }

                                    packet_received.notify_one();
                                },
                                res = sock.recv(&mut buf_tcp) => {
                                    match res {
                                        Some(size) => {
                                            if size > 0 {
                                                // Update activity on receive
                                                *last_activity.write().await = time::Instant::now();

                                                if let Err(e) = udp_sock.send(&buf_tcp[..size]).await {
                                                    error!("Unable to send UDP packet to {}: {}, closing connection", e, remote_addr);
                                                    health.store(false, Ordering::Relaxed);
                                                    quit.cancel();
                                                    return;
                                                }
                                            }
                                        },
                                        None => {
                                            // TCP connection closed, mark as unhealthy
                                            health.store(false, Ordering::Relaxed);
                                            info!("TCP connection {} closed by remote", tcp_id);
                                            quit.cancel();
                                            return;
                                        },
                                    }

                                    packet_received.notify_one();
                                },
                                _ = quit.cancelled() => {
                                    debug!("worker {} for TCP conn {} terminated", i, tcp_id);
                                    return;
                                },
                            };
                        }
                    });
                }
            }

            // Spawn keepalive task to send periodic heartbeats
            let pool_for_keepalive = pool.clone();
            let quit_for_keepalive = quit.clone();

            tokio::spawn(async move {
                let mut keepalive_interval = time::interval(KEEPALIVE_INTERVAL);

                loop {
                    tokio::select! {
                        _ = keepalive_interval.tick() => {
                            let sockets = pool_for_keepalive.get_all_sockets().await;
                            let now = time::Instant::now();

                            for (sock, id, health, last_activity) in sockets {
                                if !health.load(Ordering::Relaxed) {
                                    continue; // Skip unhealthy connections
                                }

                                let last_active = *last_activity.read().await;
                                let idle_duration = now.duration_since(last_active);

                                // Send keepalive if connection has been idle
                                if idle_duration >= KEEPALIVE_INTERVAL {
                                    debug!("Sending keepalive on connection {} (idle for {:?})", id, idle_duration);
                                    if sock.send(KEEPALIVE_PACKET).await.is_some() {
                                        *last_activity.write().await = now;
                                    } else {
                                        health.store(false, Ordering::Relaxed);
                                        debug!("Keepalive failed on connection {}, marked unhealthy", id);
                                    }
                                }
                            }
                        },
                        _ = quit_for_keepalive.cancelled() => {
                            debug!("Keepalive task terminated");
                            return;
                        }
                    }
                }
            });

            // Spawn health monitoring task
            let pool_for_health = pool.clone();
            let quit_for_health = quit.clone();

            tokio::spawn(async move {
                let mut health_check_interval = time::interval(time::Duration::from_secs(10));

                loop {
                    tokio::select! {
                        _ = health_check_interval.tick() => {
                            // Check and remove unhealthy connections
                            let remaining = pool_for_health.remove_unhealthy().await;
                            let healthy = pool_for_health.healthy_count().await;
                            let target = pool_for_health.target_count();

                            if remaining < target {
                                info!(
                                    "Connection pool health: {}/{} healthy connections remaining",
                                    healthy, target
                                );
                            }

                            // If no healthy connections remain, cancel all workers
                            if healthy == 0 && remaining == 0 {
                                info!("All connections failed, terminating");
                                quit_for_health.cancel();
                                return;
                            }
                        },
                        _ = quit_for_health.cancelled() => {
                            debug!("Health monitoring task terminated");
                            return;
                        }
                    }
                }
            });

            let connections = connections.clone();
            tokio::spawn(async move {
                loop {
                    let read_timeout = time::sleep(UDP_TTL);
                    let packet_received_fut = packet_received.notified();

                    tokio::select! {
                        _ = read_timeout => {
                            info!("No traffic seen in the last {:?}, closing connection", UDP_TTL);
                            connections.write().await.remove(&udp_remote_addr);
                            debug!("removed fake TCP socket from connections table");

                            quit.cancel();
                            return;
                        },
                        _ = quit.cancelled() => {
                            connections.write().await.remove(&udp_remote_addr);
                            debug!("removed fake TCP socket from connections table");
                            return;
                        },
                        _ = packet_received_fut => {},
                    }
                }
            });
        }
    });

    tokio::join!(main_loop).0.unwrap()
}
