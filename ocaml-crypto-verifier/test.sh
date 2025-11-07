#!/bin/bash

# Test script for OCaml crypto verifier

echo "Testing OCaml Crypto Verifier..."

# Test health endpoint
echo "Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:8001/health)
if [ $? -eq 0 ]; then
    echo "✓ Health check passed"
    echo "Response: $HEALTH_RESPONSE"
else
    echo "✗ Health check failed"
    exit 1
fi

# Test encryption verification with invalid data (should fail gracefully)
echo "Testing encryption verification endpoint..."
ENCRYPTION_PAYLOAD='{
  "algorithm": "aes256_gcm",
  "plaintext": "Hello, World!",
  "ciphertext": "dGVzdA==",  # base64 "test"
  "key": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  "nonce": "000102030405060708090a0b"
}'
ENCRYPTION_RESPONSE=$(curl -s -X POST http://localhost:8001/verify/encryption \
    -H "Content-Type: application/json" \
    -d "$ENCRYPTION_PAYLOAD")
if [ $? -eq 0 ]; then
    echo "✓ Encryption verification endpoint responded"
    echo "Response: $ENCRYPTION_RESPONSE"
else
    echo "✗ Encryption verification endpoint failed"
    exit 1
fi

# Test signature verification with invalid data (should fail gracefully)
echo "Testing signature verification endpoint..."
SIGNATURE_PAYLOAD='{
  "algorithm": "ed25519",
  "message": "Hello, World!",
  "signature": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f",
  "public_key": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
}'
SIGNATURE_RESPONSE=$(curl -s -X POST http://localhost:8001/verify/signature \
    -H "Content-Type: application/json" \
    -d "$SIGNATURE_PAYLOAD")
if [ $? -eq 0 ]; then
    echo "✓ Signature verification endpoint responded"
    echo "Response: $SIGNATURE_RESPONSE"
else
    echo "✗ Signature verification endpoint failed"
    exit 1
fi

echo "All tests passed! 🎉"
