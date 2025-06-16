#!/bin/bash
set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

mkdir -p build/compiled

COMMIT=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "dev")

echo -e "${CYAN}Building Zoi for $(uname -s)...${NC}"
go build -o "./build/compiled/zoi" \
    -ldflags "-s -w -X main.VerCommit=$COMMIT" \
    ./src

if [ $? -eq 0 ]; then
    echo -e "${GREEN}Build successful! Commit: $COMMIT${NC}"
else
    echo -e "${RED}Build failed${NC}"
    exit 1
fi