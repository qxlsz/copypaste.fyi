#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR=$(cd -- "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PROJECT_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

echo -e "${BLUE}=== Docker Build Testing Script ===${NC}\n"

# Function to print section headers
print_section() {
    echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

# Function to test a build
test_build() {
    local name=$1
    local dockerfile=$2
    local context=$3
    local tag=$4
    
    print_section "Testing: $name"
    echo -e "${YELLOW}Dockerfile:${NC} $dockerfile"
    echo -e "${YELLOW}Context:${NC} $context"
    echo -e "${YELLOW}Tag:${NC} $tag\n"
    
    if docker build -f "$dockerfile" -t "$tag" "$context"; then
        echo -e "\n${GREEN}✓ Build succeeded: $name${NC}"
        return 0
    else
        echo -e "\n${RED}✗ Build failed: $name${NC}"
        return 1
    fi
}

# Function to test a running container
test_container() {
    local name=$1
    local image=$2
    local port=$3
    local health_endpoint=$4
    
    print_section "Testing Container: $name"
    
    # Stop and remove existing container if it exists
    docker stop "$name" 2>/dev/null || true
    docker rm "$name" 2>/dev/null || true
    
    echo -e "${YELLOW}Starting container...${NC}"
    if ! docker run -d --name "$name" -p "$port:$port" "$image"; then
        echo -e "${RED}✗ Failed to start container${NC}"
        return 1
    fi
    
    echo -e "${YELLOW}Waiting for container to be ready...${NC}"
    sleep 5
    
    echo -e "${YELLOW}Testing health endpoint: $health_endpoint${NC}"
    if curl -f "$health_endpoint" 2>/dev/null; then
        echo -e "\n${GREEN}✓ Container is healthy${NC}"
        
        # Show logs
        echo -e "\n${YELLOW}Container logs:${NC}"
        docker logs "$name" --tail 20
        
        # Cleanup
        echo -e "\n${YELLOW}Cleaning up...${NC}"
        docker stop "$name"
        docker rm "$name"
        return 0
    else
        echo -e "\n${RED}✗ Health check failed${NC}"
        echo -e "\n${YELLOW}Container logs:${NC}"
        docker logs "$name"
        
        # Cleanup
        docker stop "$name"
        docker rm "$name"
        return 1
    fi
}

# Function to inspect binary in image
inspect_binary() {
    local image=$1
    local binary_path=$2
    
    print_section "Inspecting Binary in Image"
    echo -e "${YELLOW}Image:${NC} $image"
    echo -e "${YELLOW}Expected binary:${NC} $binary_path\n"
    
    echo -e "${YELLOW}Checking if binary exists...${NC}"
    if docker run --rm "$image" ls -lh "$binary_path" 2>/dev/null; then
        echo -e "${GREEN}✓ Binary found${NC}"
        
        echo -e "\n${YELLOW}Binary info:${NC}"
        docker run --rm "$image" file "$binary_path"
        
        echo -e "\n${YELLOW}Binary size:${NC}"
        docker run --rm "$image" du -h "$binary_path"
        
        return 0
    else
        echo -e "${RED}✗ Binary not found at $binary_path${NC}"
        
        echo -e "\n${YELLOW}Searching for binaries in /usr/local/bin:${NC}"
        docker run --rm "$image" ls -lh /usr/local/bin/ || true
        
        echo -e "\n${YELLOW}Searching for binaries in /home/app:${NC}"
        docker run --rm "$image" ls -lh /home/app/ || true
        
        return 1
    fi
}

# Change to project root
cd "$PROJECT_ROOT"

# Parse arguments
QUICK_MODE=false
SKIP_BACKEND=false
SKIP_OCAML=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --skip-backend)
            SKIP_BACKEND=true
            shift
            ;;
        --skip-ocaml)
            SKIP_OCAML=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --quick         Only build, skip container tests"
            echo "  --skip-backend  Skip backend Dockerfile test"
            echo "  --skip-ocaml    Skip OCaml Dockerfile test"
            echo "  --help          Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Track results
FAILED_TESTS=()

# Test 1: OCaml Crypto Verifier Dockerfile
if [[ "$SKIP_OCAML" == false ]]; then
    if ! test_build \
        "OCaml Crypto Verifier" \
        "ocaml-crypto-verifier/Dockerfile" \
        "ocaml-crypto-verifier" \
        "crypto-verifier-test"; then
        FAILED_TESTS+=("OCaml build")
    else
        # Inspect the binary
        if ! inspect_binary "crypto-verifier-test" "/home/app/crypto-verifier"; then
            FAILED_TESTS+=("OCaml binary inspection")
        fi
        
        # Test the container
        if [[ "$QUICK_MODE" == false ]]; then
            if ! test_container \
                "crypto-verifier-test-container" \
                "crypto-verifier-test" \
                "8001" \
                "http://localhost:8001/health"; then
                FAILED_TESTS+=("OCaml container test")
            fi
        fi
    fi
fi

# Test 2: Backend Dockerfile (Rust + OCaml)
if [[ "$SKIP_BACKEND" == false ]]; then
    if ! test_build \
        "Backend (Rust + OCaml)" \
        "Dockerfile.backend" \
        "." \
        "copypaste-backend-test"; then
        FAILED_TESTS+=("Backend build")
    else
        # Inspect both binaries
        if ! inspect_binary "copypaste-backend-test" "/usr/local/bin/copypaste"; then
            FAILED_TESTS+=("Backend Rust binary inspection")
        fi
        
        if ! inspect_binary "copypaste-backend-test" "/usr/local/bin/crypto-verifier"; then
            FAILED_TESTS+=("Backend OCaml binary inspection")
        fi
        
        # Test the container
        if [[ "$QUICK_MODE" == false ]]; then
            echo -e "\n${YELLOW}Note: Backend container test requires full setup (database, etc.)${NC}"
            echo -e "${YELLOW}Skipping container runtime test for backend${NC}"
        fi
    fi
fi

# Print summary
print_section "Test Summary"

if [[ ${#FAILED_TESTS[@]} -eq 0 ]]; then
    echo -e "${GREEN}✓ All tests passed!${NC}\n"
    exit 0
else
    echo -e "${RED}✗ ${#FAILED_TESTS[@]} test(s) failed:${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "${RED}  - $test${NC}"
    done
    echo ""
    exit 1
fi
