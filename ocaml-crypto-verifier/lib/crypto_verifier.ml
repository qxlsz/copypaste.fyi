type verification_result = {
  valid: bool;
  details: string;
  timestamp: float;
} [@@deriving yojson]

type encryption_verification = {
  algorithm: string;
  plaintext: string;
  ciphertext: string;
  key: string;
  nonce: string option;
  aad: string option;
} [@@deriving yojson]

type signature_verification = {
  algorithm: string;
  message: string;
  signature: string;
  public_key: string;
} [@@deriving yojson]

exception Verification_error of string

let now () = Unix.gettimeofday ()

(* Simplified verification functions - return dummy results for now *)
let verify_aes_gcm _ev =
  { valid = true; details = "AES-GCM verification placeholder"; timestamp = now () }

let verify_chacha20_poly1305 _ev =
  { valid = true; details = "ChaCha20-Poly1305 verification placeholder"; timestamp = now () }

let verify_ed25519 _sv =
  { valid = true; details = "Ed25519 signature verification placeholder"; timestamp = now () }

let verify_encryption ev =
  match String.lowercase_ascii ev.algorithm with
  | "aes256_gcm" | "aes-gcm" -> verify_aes_gcm ev
  | "chacha20_poly1305" | "chacha20-poly1305" -> verify_chacha20_poly1305 ev
  | "xchacha20_poly1305" | "xchacha20-poly1305" ->
      { valid = false; details = "XChaCha20-Poly1305 not yet implemented"; timestamp = now () }
  | alg -> { valid = false; details = "Unsupported encryption algorithm: " ^ alg; timestamp = now () }

let verify_signature sv =
  match String.lowercase_ascii sv.algorithm with
  | "ed25519" -> verify_ed25519 sv
  | alg -> { valid = false; details = "Unsupported signature algorithm: " ^ alg; timestamp = now () }

(* Health check function *)
let health_check () = { valid = true; details = "Crypto verifier is healthy"; timestamp = now () }
