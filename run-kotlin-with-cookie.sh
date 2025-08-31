#!/usr/bin/env bash

# Script to run Bitcoin Augur Kotlin reference implementation using Bitcoin Core cookie file
# Usage: ./run-kotlin-with-cookie.sh [port] [cookie_file_path]

set -e

# Default values
DEFAULT_PORT=8090
DEFAULT_COOKIE_FILE="$HOME/.bitcoin/.cookie"
KOTLIN_DIR="../bitcoin-augur-reference"

# Get port from first argument or use default
PORT="${1:-$DEFAULT_PORT}"

# Get cookie file path from second argument or use default
COOKIE_FILE="${2:-$DEFAULT_COOKIE_FILE}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Bitcoin Augur Kotlin Reference Implementation Runner${NC}"
echo "----------------------------------------"

# Check if cookie file exists
if [ ! -f "$COOKIE_FILE" ]; then
    echo -e "${RED}Error: Cookie file not found at $COOKIE_FILE${NC}"
    echo "Please ensure Bitcoin Core is running or specify a valid cookie file path"
    echo "Usage: $0 [port] [cookie_file_path]"
    exit 1
fi

# Check if Kotlin implementation directory exists
if [ ! -d "$KOTLIN_DIR" ]; then
    echo -e "${RED}Error: Kotlin implementation not found at $KOTLIN_DIR${NC}"
    echo "Please ensure the bitcoin-augur-reference directory exists"
    exit 1
fi

# Read and parse cookie file
echo -e "${YELLOW}Reading cookie file from: $COOKIE_FILE${NC}"
COOKIE_CREDS=$(cat "$COOKIE_FILE")

# Extract username and password
export BITCOIN_RPC_USERNAME=$(echo "$COOKIE_CREDS" | cut -d: -f1)
export BITCOIN_RPC_PASSWORD=$(echo "$COOKIE_CREDS" | cut -d: -f2)

# Verify credentials were extracted
if [ -z "$BITCOIN_RPC_USERNAME" ] || [ -z "$BITCOIN_RPC_PASSWORD" ]; then
    echo -e "${RED}Error: Failed to parse cookie file${NC}"
    echo "Cookie file should contain: username:password"
    exit 1
fi

echo -e "${GREEN}✓ Cookie credentials loaded${NC}"
echo "  Username: $BITCOIN_RPC_USERNAME"
echo "  Password: [hidden]"

# Set server port
export AUGUR_SERVER_PORT=$PORT
echo -e "${GREEN}✓ Server will run on port: $PORT${NC}"

# Change to Kotlin implementation directory
cd "$KOTLIN_DIR"

# Check if JAR exists for faster startup
JAR_FILE="app/build/libs/app-all.jar"
if [ -f "$JAR_FILE" ]; then
    echo -e "${YELLOW}Found pre-built JAR, using it for faster startup${NC}"
    echo "----------------------------------------"
    echo -e "${GREEN}Starting server at http://localhost:$PORT${NC}"
    echo "API Endpoints:"
    echo "  - GET http://localhost:$PORT/fees"
    echo "  - GET http://localhost:$PORT/historical_fee?timestamp=<unix_ts>"
    echo "----------------------------------------"
    
    # Check if we have Java available
    if command -v java &> /dev/null; then
        java -jar "$JAR_FILE"
    else
        echo -e "${YELLOW}Java not found in PATH, using nix-shell${NC}"
        nix-shell -p jdk17 --run "java -jar $JAR_FILE"
    fi
else
    echo -e "${YELLOW}No pre-built JAR found, building with gradle...${NC}"
    echo "----------------------------------------"
    echo -e "${GREEN}Starting server at http://localhost:$PORT${NC}"
    echo "API Endpoints:"
    echo "  - GET http://localhost:$PORT/fees"
    echo "  - GET http://localhost:$PORT/historical_fee?timestamp=<unix_ts>"
    echo "----------------------------------------"
    
    # Check if gradle wrapper exists
    if [ -x "bin/gradle" ]; then
        bin/gradle run
    else
        echo -e "${RED}Error: Gradle wrapper not found${NC}"
        echo "Please build the project first with: cd $KOTLIN_DIR && bin/gradle build"
        exit 1
    fi
fi