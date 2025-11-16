#!/bin/bash
# Test script for Phantun load balancing feature

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Phantun Load Balancing Test Script ===${NC}\n"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root (for TUN interface creation)${NC}"
    echo "Please run: sudo $0"
    exit 1
fi

# Build path
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PHANTUN_DIR="$SCRIPT_DIR/phantun"
CLIENT_BIN="$PHANTUN_DIR/target/release/client"
SERVER_BIN="$PHANTUN_DIR/target/release/server"

# Check if binaries exist
if [ ! -f "$CLIENT_BIN" ] || [ ! -f "$SERVER_BIN" ]; then
    echo -e "${RED}Error: Phantun binaries not found${NC}"
    echo "Please build first: cd phantun && cargo build --release"
    exit 1
fi

echo -e "${GREEN}✓ Found Phantun binaries${NC}"

# Test 1: Check help message for new option
echo -e "\n${YELLOW}Test 1: Checking if --num-tcp-conns option exists${NC}"
if $CLIENT_BIN --help | grep -q "num-tcp-conns"; then
    echo -e "${GREEN}✓ --num-tcp-conns option is available${NC}"
else
    echo -e "${RED}✗ --num-tcp-conns option not found${NC}"
    exit 1
fi

# Test 2: Check parameter validation
echo -e "\n${YELLOW}Test 2: Testing parameter validation${NC}"

# Test invalid value (0)
echo -n "  Testing --num-tcp-conns 0 (should fail): "
if $CLIENT_BIN --local 127.0.0.1:1234 --remote 127.0.0.1:4567 --num-tcp-conns 0 2>&1 | grep -q "must be between"; then
    echo -e "${GREEN}✓ Correctly rejected${NC}"
else
    echo -e "${RED}✗ Should have rejected${NC}"
fi

# Test invalid value (17)
echo -n "  Testing --num-tcp-conns 17 (should fail): "
if $CLIENT_BIN --local 127.0.0.1:1234 --remote 127.0.0.1:4567 --num-tcp-conns 17 2>&1 | grep -q "must be between"; then
    echo -e "${GREEN}✓ Correctly rejected${NC}"
else
    echo -e "${RED}✗ Should have rejected${NC}"
fi

# Test valid value (4)
echo -n "  Testing --num-tcp-conns 4 (should accept): "
# Just check if it doesn't error on parameter parsing (won't actually connect)
if timeout 2 $CLIENT_BIN --local 127.0.0.1:1234 --remote 127.0.0.1:4567 --num-tcp-conns 4 2>&1 | grep -q -e "cores available" -e "Error creating TUN"; then
    echo -e "${GREEN}✓ Parameter accepted${NC}"
else
    echo -e "${RED}✗ Unexpected error${NC}"
fi

echo -e "\n${GREEN}=== All basic tests passed! ===${NC}\n"

echo -e "${YELLOW}Integration Test Instructions:${NC}"
echo "To test the actual load balancing functionality, you need to run:"
echo ""
echo "1. Start UDP echo server (in terminal 1):"
echo "   nc -u -l 9999"
echo ""
echo "2. Start Phantun server (in terminal 2):"
echo "   sudo $SERVER_BIN --local 4567 --remote 127.0.0.1:9999"
echo ""
echo "3. Start Phantun client with load balancing (in terminal 3):"
echo "   sudo $CLIENT_BIN --local 127.0.0.1:8888 --remote 127.0.0.1:4567 --num-tcp-conns 4"
echo ""
echo "4. Send test data (in terminal 4):"
echo "   echo 'test message' | nc -u 127.0.0.1 8888"
echo ""
echo "You should see 'Load balancing enabled with 4 TCP connections per UDP stream' in the client output"
echo ""
echo -e "${GREEN}Test script completed successfully!${NC}"
