# Phantun Load Balancing Troubleshooting Guide

## Common Issues and Solutions

### 1. Permission Denied (EPERM)

**Error**:
```
thread 'main' panicked at phantun/src/bin/client.rs:355:10:
called `Result::unwrap()` on an `Err` value: NixError(EPERM)
```

**Cause**: Creating TUN interface requires root privileges.

**Solution**:
```bash
sudo ./bin/client --local 127.0.0.1:55555 --remote SERVER:PORT
sudo ./bin/server --local PORT --remote BACKEND:PORT
```

### 2. Client and Server Not Communicating

**Symptoms**: No data transfer between client and server.

**Possible Causes**:

**A. No UDP Traffic**
- Phantun only forwards when there's actual UDP traffic
- Check if your UDP application is sending data to the client's local address

**Verification**:
```bash
# Send test UDP packet
echo "test" | nc -u 127.0.0.1 55555
```

**B. Firewall Blocking**
```bash
# Check firewall rules
sudo iptables -L -n | grep 55555

# Temporarily disable firewall (for testing only)
sudo ufw disable
```

**C. Server Not Listening**
```bash
# Check if server is listening on the port
sudo netstat -tulpn | grep 55555
```

### 3. TUN Interface Not Created

**Check**:
```bash
ip link show | grep tun
```

**If missing**:
```bash
# Ensure TUN module is loaded
sudo modprobe tun
lsmod | grep tun
```

### 4. Connection Timeout

**Symptoms**: Client connects but times out after 180 seconds.

**Cause**: No traffic for UDP_TTL duration (180 seconds).

**Solution**: Ensure continuous UDP traffic or enable keepalive on client side.

### 5. Session Not Merging (Multiple UDP Sockets on Server)

**Check Server Logs**:
```bash
sudo RUST_LOG=info ./bin/server --local 55555 --remote 127.0.0.1:9000
```

**Expected for session mode** (--num-tcp-conns > 1):
```
Session handshake detected: session=XXXX, conn=1/4
Created new session XXXX
Added TCP connection to session XXXX, now 4 connections
```

**Expected for legacy mode** (--num-tcp-conns = 1):
```
Non-session connection detected, using legacy mode
```

**Troubleshooting**:
- If using --num-tcp-conns 1 (default), session merging is not used
- Try --num-tcp-conns 4 to test session merging
- Check that all TCP connections are established

### 6. High Latency or Packet Loss

**Check TCP Connection Quality**:
```bash
# Monitor TCP statistics
ss -ti | grep 55555
```

**Optimize**:
```bash
# Increase TCP buffer sizes
sudo sysctl -w net.core.rmem_max=26214400
sudo sysctl -w net.core.wmem_max=26214400
```

## Debug Mode

### Enable Detailed Logging

**Client**:
```bash
sudo RUST_LOG=debug ./bin/client --local 127.0.0.1:55555 --remote SERVER:55555
```

**Server**:
```bash
sudo RUST_LOG=debug ./bin/server --local 55555 --remote 127.0.0.1:9000
```

### Packet Capture

**Capture TUN Traffic**:
```bash
sudo tcpdump -i tun0 -w client_tun.pcap
```

**Capture TCP Traffic**:
```bash
sudo tcpdump -i any 'tcp port 55555' -w tcp_traffic.pcap
```

**Capture UDP Traffic**:
```bash
sudo tcpdump -i any 'udp port 55555 or udp port 9000' -w udp_traffic.pcap
```

## Testing Session Merging

### Simple Test Script

```bash
#!/bin/bash

# Terminal 1: Start UDP echo server
echo "Starting UDP echo server..."
nc -u -l 127.0.0.1 9000 &
ECHO_PID=$!

# Terminal 2: Start Phantun server
echo "Starting Phantun server..."
sudo ./bin/server --local 8080 --remote 127.0.0.1:9000 &
SERVER_PID=$!

sleep 2

# Terminal 3: Start Phantun client with 4 connections
echo "Starting Phantun client with 4 TCP connections..."
sudo ./bin/client --local 127.0.0.1:55555 --remote 127.0.0.1:8080 --num-tcp-conns 4 &
CLIENT_PID=$!

sleep 2

# Terminal 4: Send test data
echo "Sending test data..."
for i in {1..10}; do
    echo "Packet $i" | nc -u -w 1 127.0.0.1 55555
    sleep 0.5
done

# Cleanup
echo "Cleaning up..."
sudo kill $CLIENT_PID $SERVER_PID $ECHO_PID 2>/dev/null
```

### Verify Session Merging

**Check server logs for**:
```
Created UDP session XXXX with socket 0.0.0.0:RANDOM_PORT → backend 127.0.0.1:9000
Added TCP connection to session XXXX, now 2 connections
Added TCP connection to session XXXX, now 3 connections
Added TCP connection to session XXXX, now 4 connections
```

**Verify single UDP socket**:
```bash
# On server, check number of UDP sockets to backend
sudo netstat -anup | grep 9000
# Should see ONLY ONE connection for session mode
```

## Performance Testing

### Bandwidth Test

```bash
# Install iperf3
sudo apt-get install iperf3

# Server: Start iperf3 UDP server
iperf3 -s -p 9000

# Server: Start Phantun server
sudo ./bin/server --local 55555 --remote 127.0.0.1:9000

# Client: Start Phantun client
sudo ./bin/client --local 127.0.0.1:55555 --remote SERVER_IP:55555 --num-tcp-conns 4

# Client: Run iperf3 UDP test
iperf3 -c 127.0.0.1 -p 55555 -u -b 100M -t 30
```

### Compare Performance

**Single Connection**:
```bash
sudo ./bin/client --local 127.0.0.1:55555 --remote SERVER:55555 --num-tcp-conns 1
```

**Multiple Connections**:
```bash
sudo ./bin/client --local 127.0.0.1:55555 --remote SERVER:55555 --num-tcp-conns 4
```

## Getting Help

If issues persist:

1. **Collect logs**:
   ```bash
   sudo RUST_LOG=debug ./bin/client ... > client.log 2>&1
   sudo RUST_LOG=debug ./bin/server ... > server.log 2>&1
   ```

2. **Check system info**:
   ```bash
   uname -a
   cat /proc/sys/net/ipv4/ip_forward
   ```

3. **Verify network**:
   ```bash
   ping SERVER_IP
   telnet SERVER_IP 55555
   ```

4. **Create issue** with:
   - Client and server logs
   - System information
   - Network topology
   - Steps to reproduce
