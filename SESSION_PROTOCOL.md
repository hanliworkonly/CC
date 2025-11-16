# Session Protocol for Multi-TCP Load Balancing

## Implementation Status

**Phase 1 (✅ COMPLETED)**: Handshake filtering to prevent backend interference
- Client sends session handshake packets
- Server recognizes and filters handshake packets (doesn't forward to backend)
- Backend UDP application doesn't receive unexpected handshake data
- Multiple TCP connections still create separate UDP sockets (not yet merged)

**Phase 2 (🚧 TODO)**: Full session merging for true load balancing
- Server merges multiple TCP connections into single UDP socket
- Backend sees single UDP client instead of multiple
- True bandwidth aggregation across TCP paths
- Proper response distribution back to client

## Overview

When a client uses `--num-tcp-conns N` to create multiple TCP connections, the server needs to know:
1. Which TCP connections belong to the same UDP session (✅ Phase 1)
2. How to merge data from multiple TCP connections to a single UDP socket (🚧 Phase 2)
3. How to distribute UDP responses back to the TCP connections (🚧 Phase 2)

This protocol defines the handshake mechanism for session establishment.

## Protocol Design

### Handshake Packet Format

Each TCP connection sends a handshake packet immediately after establishment:

```
+----------+---------------+------------+--------------+
| Magic    | Session ID    | Conn Index | Total Conns  |
| 4 bytes  | 16 bytes      | 1 byte     | 1 byte       |
+----------+---------------+------------+--------------+
| 0-3      | 4-19          | 20         | 21           |
+----------+---------------+------------+--------------+
```

**Total size**: 22 bytes

### Field Descriptions

1. **Magic** (4 bytes): `0xDE 0xAD 0xBE 0xEF`
   - Identifies this as a session handshake packet
   - Prevents accidental interpretation of user data as handshake

2. **Session ID** (16 bytes):
   - MD5 hash of client's UDP source address (IP:PORT)
   - Same for all TCP connections from the same UDP client
   - Example: `192.168.1.100:5000` → MD5 → 16 bytes

3. **Connection Index** (1 byte):
   - 0-based index of this TCP connection
   - Range: 0 to (Total Conns - 1)
   - Used for debugging and monitoring

4. **Total Connections** (1 byte):
   - How many TCP connections in this session
   - Range: 1-255
   - Allows server to know when all connections are established

### Handshake Sequence

```
Client UDP: 192.168.1.100:5000
Client starts with --num-tcp-conns 3

Timeline:
T0: TCP connection 1 established → Send handshake(session_id, 0, 3)
T1: TCP connection 2 established → Send handshake(session_id, 1, 3)
T2: TCP connection 3 established → Send handshake(session_id, 2, 3)
T3: Server recognizes session complete (3/3 connections)
T4: All subsequent data packets sent normally (no header)
```

### Server-Side Session Management

The server maintains a session table:

```rust
struct UdpSession {
    session_id: [u8; 16],
    udp_socket: Arc<UdpSocket>,
    tcp_connections: Vec<Arc<Socket>>,
    next_tcp_idx: AtomicUsize,  // For round-robin UDP→TCP distribution
    remote_addr: SocketAddr,     // Backend UDP server
}

// Global session table
sessions: Arc<RwLock<HashMap<[u8; 16], Arc<UdpSession>>>>
```

### Data Flow After Handshake

**Client → Server (TCP → UDP)**:
```
TCP Conn 0: [UDP packet 0] →
TCP Conn 1: [UDP packet 1] →  } → Merged to single UDP socket → Backend
TCP Conn 2: [UDP packet 2] →
```

**Server → Client (UDP → TCP)**:
```
Backend → Single UDP socket → Server receives packet
                            → Round-robin to TCP Conn (0, 1, or 2)
                            → Client merges back to UDP
```

## Backward Compatibility

**When client uses `--num-tcp-conns 1` (default)**:
- No handshake packet sent
- Server creates UDP socket per TCP connection (original behavior)
- Fully backward compatible

**When server doesn't support sessions**:
- Handshake packet treated as regular data (22 bytes forwarded to backend)
- Multiple UDP sockets created (sub-optimal but won't crash)
- Backend may reject malformed data

## Security Considerations

1. **Session ID Collision**:
   - MD5 collision probability negligible for this use case
   - Different clients will have different UDP source addresses

2. **Handshake Spoofing**:
   - Attacker can't easily guess session_id without knowing client's UDP address
   - Phantom's existing security model (UDP source validation) still applies

3. **DoS via Session Creation**:
   - Server should limit max sessions (e.g., 10,000)
   - Implement session timeout and cleanup

## Configuration

### Client Parameters
```bash
./client --local 127.0.0.1:4567 \
         --remote SERVER:8080 \
         --num-tcp-conns 4      # Triggers session protocol
```

### Server Parameters
```bash
./server --local 8080 \
         --remote 10.0.0.1:9000
         # Session support auto-enabled
```

## Implementation Notes

1. **Handshake Timing**: Sent immediately after TCP connection established, before any user data
2. **Idempotency**: Server should handle duplicate handshakes gracefully (reconnection scenario)
3. **Session Cleanup**: Remove session when all TCP connections close or after timeout
4. **Monitoring**: Log session creation/destruction for debugging

## Example Session Lifecycle

```
[Client] UDP app sends packet to 127.0.0.1:4567
[Client] Phantun client receives, checks connection pool
[Client] Pool empty, creates 4 TCP connections to server
[Client] Each connection sends handshake(session_id=ABC123, idx=0-3, total=4)
[Server] Receives handshakes, creates session ABC123
[Server] Creates single UDP socket for session ABC123
[Server] Links all 4 TCP connections to this session
[Client] Sends UDP packet 1 via TCP conn 0
[Server] Receives on TCP conn 0, looks up session ABC123, sends via shared UDP socket
[Backend] Receives UDP packet from single source port
[Backend] Replies
[Server] Receives reply on shared UDP socket, round-robin to TCP conn 1
[Client] Receives on TCP conn 1, forwards to local UDP app
...
[Client] UDP app closes
[Client] Closes all 4 TCP connections
[Server] Detects all connections closed, cleans up session ABC123
```

## Benefits

1. **Correct Session Semantics**: Backend sees single UDP client
2. **True Load Balancing**: Bandwidth aggregated across multiple TCP paths
3. **Response Handling**: Proper distribution of backend replies
4. **State Preservation**: Stateful protocols (QUIC, VPN, games) work correctly

## Performance Impact

- **Handshake Overhead**: 22 bytes × N connections (one-time per session)
- **Memory**: ~200 bytes per session (session table entry)
- **CPU**: Hash lookup O(1) for each packet
- **Latency**: No additional latency after handshake

For `--num-tcp-conns 4`:
- Handshake: 88 bytes total (22 × 4)
- Amortized over typical session: negligible (<0.001%)

## Current Limitations (Phase 1)

Since full session merging (Phase 2) is not yet implemented, the current system has these limitations:

1. **Backend sees multiple UDP clients**: Each TCP connection creates its own UDP socket on the server side
   - Example: `--num-tcp-conns 4` results in 4 different UDP source ports to the backend
   - Stateful protocols (VPN, QUIC, games) may not work correctly

2. **No true load balancing**: Bandwidth is distributed across TCP connections, but responses come back through separate UDP sockets
   - Upload bandwidth: aggregated ✅
   - Download bandwidth: NOT aggregated ❌

3. **Session tracking incomplete**: Server recognizes session handshakes but doesn't maintain session state
   - Handshakes are filtered correctly ✅
   - No session timeout or cleanup yet ❌

## Current Use Cases (Phase 1)

The current implementation is suitable for:

✅ **Testing multi-TCP client functionality**: Verify client can create and manage multiple connections
✅ **Stateless UDP protocols**: Simple request-response protocols where source port doesn't matter
✅ **Upload-heavy workloads**: Uploading large files where download responses are minimal

❌ **Not yet suitable for**:
- Stateful protocols (VPN, QUIC, multiplayer games)
- Bidirectional high-throughput applications
- Production environments requiring session semantics

## Roadmap to Phase 2

To complete full session merging, the following work is needed:

1. **Server session table**: Maintain `session_id` → `UdpSession` mapping
2. **UDP socket sharing**: Multiple TCP connections write to same UDP socket
3. **Response distribution**: Round-robin or smart distribution of UDP responses to TCP connections
4. **Session lifecycle**: Creation, timeout monitoring, and cleanup
5. **Reconnection handling**: Add failed TCP connections back to session
6. **Testing**: Verify stateful protocols work correctly

Estimated complexity: ~300 lines of additional server code + testing

## Contributing

If you'd like to help implement Phase 2, please see the TODO comments in `phantun/phantun/src/bin/server.rs` around the `UdpSession` struct.
