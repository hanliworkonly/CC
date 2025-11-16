use clap::{crate_version, Arg, ArgAction, Command};
use fake_tcp::packet::MAX_PACKET_LEN;
use fake_tcp::{Socket, Stack};
use log::{debug, error, info, warn};
use phantun::utils::{assign_ipv6_address, new_udp_reuseport};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{Notify, RwLock};
use tokio::time;
use tokio_tun::TunBuilder;
use tokio_util::sync::CancellationToken;

use phantun::UDP_TTL;

// Session protocol configuration for multi-TCP load balancing
const SESSION_MAGIC: &[u8; 4] = b"\xDE\xAD\xBE\xEF"; // Magic bytes to identify handshake
const SESSION_HANDSHAKE_SIZE: usize = 22; // 4 (magic) + 16 (session_id) + 1 (index) + 1 (total)

// UDP session that aggregates multiple TCP connections
struct UdpSession {
    session_id: [u8; 16],
    udp_socket: Arc<UdpSocket>,
    tcp_connections: Arc<RwLock<Vec<Arc<Socket>>>>,
    next_tcp_idx: AtomicUsize,  // For round-robin UDP→TCP distribution
    #[allow(dead_code)] // Reserved for future reconnection/monitoring features
    remote_addr: SocketAddr,
    packet_received: Arc<Notify>,
    quit: CancellationToken,
}

impl UdpSession {
    async fn new(
        session_id: [u8; 16],
        _remote_addr: SocketAddr,
        backend_addr: SocketAddr,
    ) -> io::Result<Self> {
        let udp_sock = UdpSocket::bind(if backend_addr.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        }).await?;

        udp_sock.connect(backend_addr).await?;

        info!("Created UDP session {:02x?} with socket {} → backend {}",
              &session_id[0..4], udp_sock.local_addr()?, backend_addr);

        Ok(Self {
            session_id,
            udp_socket: Arc::new(udp_sock),
            tcp_connections: Arc::new(RwLock::new(Vec::new())),
            next_tcp_idx: AtomicUsize::new(0),
            remote_addr: backend_addr,
            packet_received: Arc::new(Notify::new()),
            quit: CancellationToken::new(),
        })
    }

    async fn add_connection(&self, sock: Arc<Socket>) {
        let mut conns = self.tcp_connections.write().await;
        conns.push(sock.clone());
        info!("Added TCP connection to session {:02x?}, now {} connections",
              &self.session_id[0..4], conns.len());
    }

    async fn get_next_tcp(&self) -> Option<Arc<Socket>> {
        let conns = self.tcp_connections.read().await;
        if conns.is_empty() {
            return None;
        }
        let idx = self.next_tcp_idx.fetch_add(1, Ordering::Relaxed) % conns.len();
        Some(conns[idx].clone())
    }
}

// Parse session handshake packet
fn parse_handshake(buf: &[u8]) -> Option<([u8; 16], u8, u8)> {
    if buf.len() != SESSION_HANDSHAKE_SIZE {
        return None;
    }

    // Check magic bytes
    if &buf[0..4] != SESSION_MAGIC {
        return None;
    }

    // Extract session ID
    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&buf[4..20]);

    // Extract connection index and total
    let conn_index = buf[20];
    let total_conns = buf[21];

    Some((session_id, conn_index, total_conns))
}

#[tokio::main]
async fn main() -> io::Result<()> {
    pretty_env_logger::init();

    let matches = Command::new("Phantun Server")
        .version(crate_version!())
        .author("Datong Sun (github.com/dndx)")
        .arg(
            Arg::new("local")
                .short('l')
                .long("local")
                .required(true)
                .value_name("PORT")
                .help("Sets the port where Phantun Server listens for incoming Phantun Client TCP connections")
        )
        .arg(
            Arg::new("remote")
                .short('r')
                .long("remote")
                .required(true)
                .value_name("IP or HOST NAME:PORT")
                .help("Sets the address or host name and port where Phantun Server forwards UDP packets to, IPv6 address need to be specified as: \"[IPv6]:PORT\"")
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
                .help("Sets the Tun interface local address (O/S's end)")
                .default_value("192.168.201.1")
        )
        .arg(
            Arg::new("tun_peer")
                .long("tun-peer")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface destination (peer) address (Phantun Server's end). \
                       You will need to setup DNAT rules to this address in order for Phantun Server \
                       to accept TCP traffic from Phantun Client")
                .default_value("192.168.201.2")
        )
        .arg(
            Arg::new("ipv4_only")
                .long("ipv4-only")
                .short('4')
                .required(false)
                .help("Do not assign IPv6 addresses to Tun interface")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["tun_local6", "tun_peer6"]),
        )
        .arg(
            Arg::new("tun_local6")
                .long("tun-local6")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv6 local address (O/S's end)")
                .default_value("fcc9::1")
        )
        .arg(
            Arg::new("tun_peer6")
                .long("tun-peer6")
                .required(false)
                .value_name("IP")
                .help("Sets the Tun interface IPv6 destination (peer) address (Phantun Client's end). \
                       You will need to setup SNAT/MASQUERADE rules on your Internet facing interface \
                       in order for Phantun Client to connect to Phantun Server")
                .default_value("fcc9::2")
        )
        .arg(
            Arg::new("handshake_packet")
                .long("handshake-packet")
                .required(false)
                .value_name("PATH")
                .help("Specify a file, which, after TCP handshake, its content will be sent as the \
                      first data packet to the client.\n\
                      Note: ensure this file's size does not exceed the MTU of the outgoing interface. \
                      The content is always sent out in a single packet and will not be further segmented")
        )
        .get_matches();

    let local_port: u16 = matches
        .get_one::<String>("local")
        .unwrap()
        .parse()
        .expect("bad local port");

    let remote_addr = tokio::net::lookup_host(matches.get_one::<String>("remote").unwrap())
        .await
        .expect("bad remote address or host")
        .next()
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

    if let (Some(tun_local6), Some(tun_peer6)) = (tun_local6, tun_peer6) {
        assign_ipv6_address(tun[0].name(), tun_local6, tun_peer6);
    }

    info!("Created TUN device {}", tun[0].name());

    //thread::sleep(time::Duration::from_secs(5));
    let mut stack = Stack::new(tun, tun_local, tun_local6);
    stack.listen(local_port);
    info!("Listening on {}", local_port);

    let main_loop = tokio::spawn(async move {
        // Session table for multi-TCP load balancing
        let sessions: Arc<RwLock<HashMap<[u8; 16], Arc<UdpSession>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        loop {
            let sock = Arc::new(stack.accept().await);
            info!("New connection: {}", sock);

            // Send optional compatibility handshake packet
            if let Some(ref p) = handshake_packet {
                if sock.send(p).await.is_none() {
                    error!("Failed to send handshake packet to remote, closing connection.");
                    continue;
                }
                debug!("Sent handshake packet to: {}", sock);
            }

            let sessions_clone = sessions.clone();
            let remote_addr_clone = remote_addr;
            let num_cpus_clone = num_cpus;

            // Spawn task to handle this connection
            tokio::spawn(async move {
                let mut buf_tcp = [0u8; MAX_PACKET_LEN];

                // Read first packet to determine if this is a session connection
                let first_packet = match sock.recv(&mut buf_tcp).await {
                    Some(size) if size > 0 => Some((buf_tcp[..size].to_vec(), size)),
                    _ => {
                        warn!("Connection closed immediately after accept");
                        return;
                    }
                };

                if let Some((first_pkt_vec, _first_pkt_size)) = first_packet {
                    // Try to parse as session handshake
                    if let Some((session_id, conn_index, total_conns)) = parse_handshake(&first_pkt_vec) {
                        info!("Session handshake detected: session={:02x?}, conn={}/{}",
                              &session_id[0..4], conn_index + 1, total_conns);

                        // Get or create session
                        let session = {
                            let mut sessions_map = sessions_clone.write().await;
                            if let Some(existing_session) = sessions_map.get(&session_id) {
                                debug!("Joining existing session {:02x?}", &session_id[0..4]);
                                existing_session.clone()
                            } else {
                                match UdpSession::new(session_id, remote_addr_clone, remote_addr_clone).await {
                                    Ok(new_session) => {
                                        let session_arc = Arc::new(new_session);
                                        sessions_map.insert(session_id, session_arc.clone());
                                        info!("Created new session {:02x?}", &session_id[0..4]);

                                        // Start UDP→TCP forwarding workers for this new session
                                        for i in 0..num_cpus_clone {
                                            let session_worker = session_arc.clone();
                                            tokio::spawn(async move {
                                                let mut buf = [0u8; MAX_PACKET_LEN];
                                                loop {
                                                    tokio::select! {
                                                        Ok(size) = session_worker.udp_socket.recv(&mut buf) => {
                                                            if let Some(tcp_sock) = session_worker.get_next_tcp().await {
                                                                if tcp_sock.send(&buf[..size]).await.is_some() {
                                                                    session_worker.packet_received.notify_one();
                                                                }
                                                            }
                                                        },
                                                        _ = session_worker.quit.cancelled() => {
                                                            debug!("Session {:02x?} UDP→TCP worker {} terminated",
                                                                   &session_worker.session_id[0..4], i);
                                                            return;
                                                        }
                                                    }
                                                }
                                            });
                                        }

                                        // Start session timeout monitor
                                        let session_monitor = session_arc.clone();
                                        let session_id_copy = session_id;
                                        let sessions_for_cleanup = sessions_clone.clone();
                                        tokio::spawn(async move {
                                            loop {
                                                let read_timeout = time::sleep(UDP_TTL);
                                                let packet_received_fut = session_monitor.packet_received.notified();

                                                tokio::select! {
                                                    _ = read_timeout => {
                                                        info!("Session {:02x?} timeout after {:?}, closing",
                                                              &session_id_copy[0..4], UDP_TTL);
                                                        session_monitor.quit.cancel();
                                                        sessions_for_cleanup.write().await.remove(&session_id_copy);
                                                        return;
                                                    },
                                                    _ = packet_received_fut => {},
                                                }
                                            }
                                        });

                                        session_arc
                                    },
                                    Err(e) => {
                                        error!("Failed to create UDP session: {}", e);
                                        return;
                                    }
                                }
                            }
                        };

                        // Add this TCP connection to the session
                        session.add_connection(sock.clone()).await;

                        // Start TCP→UDP forwarding for this connection
                        tokio::spawn(async move {
                            let mut buf = [0u8; MAX_PACKET_LEN];
                            loop {
                                tokio::select! {
                                    res = sock.recv(&mut buf) => {
                                        match res {
                                            Some(size) if size > 0 => {
                                                if let Err(e) = session.udp_socket.send(&buf[..size]).await {
                                                    error!("Failed to send to UDP backend: {}", e);
                                                    return;
                                                }
                                                session.packet_received.notify_one();
                                            },
                                            _ => {
                                                debug!("TCP connection closed in session {:02x?}",
                                                       &session.session_id[0..4]);
                                                return;
                                            }
                                        }
                                    },
                                    _ = session.quit.cancelled() => {
                                        debug!("Session quit signal received");
                                        return;
                                    }
                                }
                            }
                        });

                    } else {
                        // Not a session handshake - use legacy single-connection mode
                        info!("Non-session connection detected, using legacy mode");

                        let packet_received = Arc::new(Notify::new());
                        let quit = CancellationToken::new();

                        let udp_sock = match UdpSocket::bind(if remote_addr_clone.is_ipv4() {
                            "0.0.0.0:0"
                        } else {
                            "[::]:0"
                        }).await {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Failed to bind UDP socket: {}", e);
                                return;
                            }
                        };

                        let local_addr = match udp_sock.local_addr() {
                            Ok(addr) => addr,
                            Err(e) => {
                                error!("Failed to get local address: {}", e);
                                return;
                            }
                        };
                        drop(udp_sock);

                        // Forward the first packet (it's actual data, not handshake)
                        let first_packet_data = first_pkt_vec;

                        for i in 0..num_cpus_clone {
                            let sock_worker = sock.clone();
                            let quit_worker = quit.clone();
                            let packet_received_worker = packet_received.clone();
                            let udp_sock = new_udp_reuseport(local_addr);
                            let first_data = if i == 0 { Some(first_packet_data.clone()) } else { None };

                            tokio::spawn(async move {
                                let mut buf_worker_tcp = [0u8; MAX_PACKET_LEN];
                                let mut buf_worker_udp = [0u8; MAX_PACKET_LEN];

                                if udp_sock.connect(remote_addr_clone).await.is_err() {
                                    error!("Failed to connect UDP socket");
                                    return;
                                }

                                // Send first packet if this is worker 0
                                if let Some(data) = first_data {
                                    if let Err(e) = udp_sock.send(&data).await {
                                        error!("Failed to send first packet to UDP backend: {}", e);
                                        quit_worker.cancel();
                                        return;
                                    }
                                    packet_received_worker.notify_one();
                                }

                                loop {
                                    tokio::select! {
                                        Ok(size) = udp_sock.recv(&mut buf_worker_udp) => {
                                            if sock_worker.send(&buf_worker_udp[..size]).await.is_none() {
                                                quit_worker.cancel();
                                                return;
                                            }
                                            packet_received_worker.notify_one();
                                        },
                                        res = sock_worker.recv(&mut buf_worker_tcp) => {
                                            match res {
                                                Some(size) if size > 0 => {
                                                    if let Err(e) = udp_sock.send(&buf_worker_tcp[..size]).await {
                                                        error!("Unable to send UDP packet: {}", e);
                                                        quit_worker.cancel();
                                                        return;
                                                    }
                                                },
                                                _ => {
                                                    quit_worker.cancel();
                                                    return;
                                                }
                                            }
                                            packet_received_worker.notify_one();
                                        },
                                        _ = quit_worker.cancelled() => {
                                            debug!("Legacy worker {} terminated", i);
                                            return;
                                        }
                                    }
                                }
                            });
                        }

                        // Timeout monitor for legacy mode
                        tokio::spawn(async move {
                            loop {
                                let read_timeout = time::sleep(UDP_TTL);
                                let packet_received_fut = packet_received.notified();

                                tokio::select! {
                                    _ = read_timeout => {
                                        info!("No traffic seen in the last {:?}, closing connection", UDP_TTL);
                                        quit.cancel();
                                        return;
                                    },
                                    _ = packet_received_fut => {},
                                }
                            }
                        });
                    }
                }
            });
        }
    });

    tokio::join!(main_loop).0.unwrap()
}
