# OCaml Crypto Verifier

A cryptographic verification service written in OCaml for copypaste.fyi. This service provides independent verification of cryptographic operations to ensure the integrity and correctness of encryption, decryption, and signature verification processes.

## Features

- **AES-256-GCM** verification
- **ChaCha20-Poly1305** verification
- **Ed25519** signature verification
- REST API with JSON endpoints
- Docker containerized deployment
- Health check endpoint

## API Endpoints

### GET /health
Returns the health status of the service.

**Response:**
```json
{
  "status": "healthy",
  "verifier": {
    "valid": true,
    "details": "Crypto verifier is healthy",
    "timestamp": 1640995200.0
  }
}
```

### POST /verify/encryption
Verifies encryption operations.

**Request:**
```json
{
  "algorithm": "aes256_gcm",
  "plaintext": "Hello, World!",
  "ciphertext": "base64_encoded_ciphertext",
  "key": "hex_encoded_key",
  "nonce": "hex_encoded_nonce",
  "aad": "additional_authenticated_data"
}
```

**Response:**
```json
{
  "valid": true,
  "details": "AES-GCM verification successful",
  "timestamp": 1640995200.0
}
```

Supported algorithms: `aes256_gcm`, `chacha20_poly1305`, `xchacha20_poly1305`

### POST /verify/signature
Verifies digital signatures.

**Request:**
```json
{
  "algorithm": "ed25519",
  "message": "Hello, World!",
  "signature": "hex_encoded_signature",
  "public_key": "hex_encoded_public_key"
}
```

**Response:**
```json
{
  "valid": true,
  "details": "Ed25519 signature verification successful",
  "timestamp": 1640995200.0
}
```

Supported algorithms: `ed25519`

## Building and Running

### Local Development

```bash
# Install dependencies
opam install . --deps-only --yes

# Build
dune build

# Run tests
dune test

# Run server
dune exec bin/server.exe
```

### Docker

```bash
# Build image
docker build -t crypto-verifier .

# Run container
docker run -p 8001:8001 crypto-verifier
```

### Docker Compose

```bash
# Start all services including the crypto verifier
docker compose up --build
```

## Integration with Rust Backend

The OCaml crypto verifier can be integrated with the Rust backend to provide an additional layer of cryptographic verification:

```rust
// Example integration (pseudo-code)
async fn verify_encryption_with_ocaml(params: VerificationParams) -> Result<bool, Error> {
    let client = reqwest::Client::new();
    let response = client
        .post("http://crypto-verifier:8001/verify/encryption")
        .json(&params)
        .send()
        .await?;

    let result: VerificationResult = response.json().await?;
    Ok(result.valid)
}
```

## Security Considerations

- This service should run in a separate container/network from the main application
- All cryptographic keys and sensitive data are processed in memory only
- The service provides verification but does not store any persistent state
- Consider rate limiting and authentication for production deployments

## Dependencies

- `mirage-crypto`: Cryptographic primitives
- `cohttp-lwt-unix`: HTTP server
- `yojson`: JSON handling
- `logs`: Logging framework
- `ounit2`: Unit testing

## Development

### Adding New Algorithms

1. Add the algorithm name to the supported list in `crypto_verifier.ml`
2. Implement the verification function using appropriate mirage-crypto modules
3. Add tests in `crypto_verifier_test.ml`
4. Update API documentation

### Testing

```bash
# Run unit tests
dune test

# Run with coverage (requires bisect_ppx)
dune test --instrument-with bisect_ppx
bisect-ppx-report html
```

## License

MIT License - see the main project LICENSE file.
