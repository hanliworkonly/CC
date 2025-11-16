# UDP to Multiple TCP Streams Load Balancing

## Overview

This implementation adds load balancing capability to Phantun, enabling a single UDP stream to be distributed across multiple TCP connections. This feature improves throughput and bandwidth utilization.

## Architecture

### Design Principle

The load balancing is implemented on the **client side** using a connection pool with round-robin distribution:

1. **Connection Pool**: For each UDP source, the client creates multiple TCP connections (configurable)
2. **Round-Robin Distribution**: UDP packets are distributed across TCP connections in a round-robin fashion
3. **Server Compatibility**: Server side requires no changes - it naturally accepts multiple TCP connections

### Key Components

#### ConnectionPool Structure
```rust
struct ConnectionPool {
    sockets: Vec<Arc<Socket>>,     // Pool of TCP connections
    next_idx: AtomicUsize,          // Atomic counter for round-robin
}
```

#### Load Balancing Flow

```
UDP Application
      |
      v
[Phantun Client] ---> Creates N TCP connections
      |                (round-robin packet distribution)
      |
      +---> TCP Conn 1 ---+
      +---> TCP Conn 2 ---+---> [Phantun Server] ---> UDP Server
      +---> TCP Conn N ---+
```

## Usage

### Command Line Options

#### Client

Add the `--num-tcp-conns` parameter to specify the number of TCP connections:

```bash
# Single connection (default behavior)
phantun_client --local 127.0.0.1:1234 --remote 10.0.0.1:4567

# Load balancing with 4 TCP connections
phantun_client --local 127.0.0.1:1234 --remote 10.0.0.1:4567 --num-tcp-conns 4
```

**Parameters:**
- `--num-tcp-conns NUM`: Number of TCP connections (1-16, default: 1)
  - 1: No load balancing (original behavior)
  - 2-8: Recommended range for load balancing
  - Maximum: 16 connections

#### Server

No changes required. Server automatically handles multiple connections:

```bash
phantun_server --local 4567 --remote 127.0.0.1:1234
```

### Example Setup

**Scenario**: Load balance WireGuard traffic over 4 TCP connections

**Client:**
```bash
RUST_LOG=info phantun_client \
  --local 127.0.0.1:51820 \
  --remote vpn.example.com:4567 \
  --num-tcp-conns 4
```

**Server:**
```bash
RUST_LOG=info phantun_server \
  --local 4567 \
  --remote 127.0.0.1:51820
```

## Performance Considerations

### Benefits

1. **Increased Throughput**: Multiple TCP connections can aggregate bandwidth
2. **Better Utilization**: Distributes load across multiple paths
3. **Improved Reliability**: Failure of one connection doesn't stop all traffic

### Worker Scaling

- Each TCP connection spawns `num_cpus` workers
- Total workers = `num_tcp_conns × num_cpus`
- Example: 4 connections on 8-core system = 32 workers

### Recommended Settings

| Use Case | Recommended Connections | Notes |
|----------|------------------------|-------|
| Low bandwidth (<10 Mbps) | 1-2 | Overhead not worth it |
| Medium bandwidth (10-100 Mbps) | 2-4 | Good balance |
| High bandwidth (>100 Mbps) | 4-8 | Maximum benefit |
| Very high bandwidth (>1 Gbps) | 6-16 | Diminishing returns >8 |

## Implementation Details

### Round-Robin Algorithm

```rust
fn get_next(&self) -> &Arc<Socket> {
    let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) % self.sockets.len();
    &self.sockets[idx]
}
```

- Uses atomic operations for thread-safety
- No locks needed for packet distribution
- Equal distribution across all connections

### UDP Semantics Preservation

The implementation preserves UDP characteristics:

1. **Out-of-order delivery**: Maintained naturally as packets go through different TCP connections
2. **No flow control**: Each TCP connection operates independently
3. **No retransmission**: Fake TCP doesn't implement retransmission

## Limitations

1. **Packet Ordering**: Packets may arrive out of order (this is expected for UDP)
2. **Connection Overhead**: Each TCP connection has handshake overhead
3. **Resource Usage**: More connections = more memory and CPU usage
4. **NAT/Firewall**: Some devices may limit concurrent connections

## Testing

### Basic Test

1. Start server:
   ```bash
   sudo phantun_server --local 4567 --remote 127.0.0.1:9999
   ```

2. Start UDP echo server:
   ```bash
   nc -u -l 9999
   ```

3. Start client with load balancing:
   ```bash
   sudo phantun_client --local 127.0.0.1:8888 --remote 127.0.0.1:4567 --num-tcp-conns 4
   ```

4. Send test traffic:
   ```bash
   nc -u 127.0.0.1 8888
   ```

### Performance Test

Use `iperf3` for throughput testing:

```bash
# Server side
iperf3 -s -p 5201

# Phantun server
sudo phantun_server --local 4567 --remote 127.0.0.1:5201

# Phantun client (with load balancing)
sudo phantun_client --local 127.0.0.1:5202 --remote SERVER_IP:4567 --num-tcp-conns 4

# Client test
iperf3 -c 127.0.0.1 -p 5202 -u -b 100M
```

## Troubleshooting

### Problem: Not seeing performance improvement

**Solutions:**
- Check if network path supports multiple flows
- Verify `num_cpus` is sufficient
- Monitor CPU usage (may be CPU-bound)
- Try different connection counts

### Problem: High CPU usage

**Solutions:**
- Reduce `num-tcp-conns`
- Check if system has enough cores
- Monitor with `htop` or `top`

### Problem: Connection failures

**Solutions:**
- Check firewall rules allow all connections
- Verify NAT supports multiple concurrent connections
- Check server resource limits (ulimit -n)

## Future Enhancements

Potential improvements for future versions:

1. **Dynamic connection scaling**: Automatically adjust connection count based on load
2. **Weighted distribution**: Assign different weights to connections
3. **Connection health monitoring**: Detect and avoid failed connections
4. **Adaptive algorithms**: Hash-based or latency-based distribution
5. **Per-packet statistics**: Monitor distribution across connections

## References

- Original Phantun: https://github.com/dndx/phantun
- Feature request: Listed in "Future plans" section of README
- Design discussion: This implementation follows the connection pool approach
