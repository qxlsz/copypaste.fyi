type verification_result = {
  valid: bool;
  details: string;
  timestamp: float;
}

type encryption_verification = {
  algorithm: string;
  plaintext: string;
  ciphertext: string;
  key: string;
  nonce: string option;
  salt: string option;
  aad: string option;
}

type signature_verification = {
  algorithm: string;
  message: string;
  signature: string;
  public_key: string;
}

exception Verification_error of string

let now () = Unix.gettimeofday ()

(* KDF: SHA-256(salt || passphrase) -> 32 bytes, matching Rust derive_key_material *)
let derive_key ~salt ~passphrase =
  salt ^ passphrase
  |> Digestif.SHA256.digest_string
  |> Digestif.SHA256.to_raw_string

let verify_aes_gcm ev =
  match ev.nonce, ev.salt with
  | None, _ -> { valid = false; details = "Missing nonce"; timestamp = now () }
  | _, None -> { valid = false; details = "Missing salt"; timestamp = now () }
  | Some nonce_b64, Some salt_b64 ->
    (try
      let ct_bytes = Base64.decode_exn ev.ciphertext in
      let nonce_bytes = Base64.decode_exn nonce_b64 in
      let salt_bytes = Base64.decode_exn salt_b64 in
      let key_material = derive_key ~salt:salt_bytes ~passphrase:ev.key in
      let key = Mirage_crypto.AES.GCM.of_secret key_material in
      (match Mirage_crypto.AES.GCM.authenticate_decrypt
        ~key ~nonce:nonce_bytes ~adata:"" ct_bytes with
      | Some decrypted ->
        if decrypted = ev.plaintext then
          { valid = true; details = "AES-GCM verification passed"; timestamp = now () }
        else
          { valid = false; details = "Plaintext mismatch after decryption"; timestamp = now () }
      | None ->
        { valid = false; details = "AES-GCM authentication tag invalid"; timestamp = now () })
    with e ->
      { valid = false; details = "AES-GCM verification error: " ^ Printexc.to_string e; timestamp = now () })

let verify_chacha20_poly1305 ev =
  match ev.nonce, ev.salt with
  | None, _ -> { valid = false; details = "Missing nonce"; timestamp = now () }
  | _, None -> { valid = false; details = "Missing salt"; timestamp = now () }
  | Some nonce_b64, Some salt_b64 ->
    (try
      let ct_bytes = Base64.decode_exn ev.ciphertext in
      let nonce_bytes = Base64.decode_exn nonce_b64 in
      let salt_bytes = Base64.decode_exn salt_b64 in
      let key_material = derive_key ~salt:salt_bytes ~passphrase:ev.key in
      (match Mirage_crypto.Chacha20.authenticate_decrypt
        ~key:(Mirage_crypto.Chacha20.of_secret key_material)
        ~nonce:nonce_bytes ct_bytes with
      | Some decrypted ->
        if decrypted = ev.plaintext then
          { valid = true; details = "ChaCha20-Poly1305 verification passed"; timestamp = now () }
        else
          { valid = false; details = "Plaintext mismatch after decryption"; timestamp = now () }
      | None ->
        { valid = false; details = "ChaCha20-Poly1305 authentication tag invalid"; timestamp = now () })
    with e ->
      { valid = false; details = "ChaCha20-Poly1305 verification error: " ^ Printexc.to_string e; timestamp = now () })

let verify_ed25519 sv =
  try
    let sig_bytes = Base64.decode_exn sv.signature in
    let pk_bytes = Base64.decode_exn sv.public_key in
    match Mirage_crypto_ec.Ed25519.pub_of_octets pk_bytes with
    | Error _ ->
        { valid = false; details = "Invalid Ed25519 public key"; timestamp = now () }
    | Ok pub ->
        if Mirage_crypto_ec.Ed25519.verify ~key:pub sig_bytes ~msg:sv.message then
          { valid = true; details = "Ed25519 signature verified"; timestamp = now () }
        else
          { valid = false; details = "Ed25519 signature invalid"; timestamp = now () }
  with e ->
    { valid = false; details = "Ed25519 verification error: " ^ Printexc.to_string e; timestamp = now () }

let verify_encryption (ev : encryption_verification) : verification_result =
  match String.lowercase_ascii ev.algorithm with
  | "aes256_gcm" | "aes-gcm" -> verify_aes_gcm ev
  | "chacha20_poly1305" | "chacha20-poly1305" -> verify_chacha20_poly1305 ev
  | "xchacha20_poly1305" | "xchacha20-poly1305" ->
      { valid = false; details = "XChaCha20-Poly1305 not yet implemented"; timestamp = now () }
  | alg -> { valid = false; details = "Unsupported encryption algorithm: " ^ alg; timestamp = now () }

let verify_signature (sv : signature_verification) : verification_result =
  match String.lowercase_ascii sv.algorithm with
  | "ed25519" -> verify_ed25519 sv
  | alg -> { valid = false; details = "Unsupported signature algorithm: " ^ alg; timestamp = now () }

(* Health check function *)
let health_check () = { valid = true; details = "Crypto verifier is healthy"; timestamp = now () }

(* JSON parsing *)
let encryption_verification_of_json json =
  let open Yojson.Safe.Util in
  {
    algorithm = json |> member "algorithm" |> to_string;
    plaintext = json |> member "plaintext" |> to_string;
    ciphertext = json |> member "ciphertext" |> to_string;
    key = json |> member "key" |> to_string;
    nonce = json |> member "nonce" |> to_option to_string;
    salt = json |> member "salt" |> to_option to_string;
    aad = json |> member "aad" |> to_option to_string;
  }

let encryption_verification_of_string s =
  try Ok (Yojson.Safe.from_string s |> encryption_verification_of_json)
  with e -> Error (Printexc.to_string e)

let signature_verification_of_json json =
  let open Yojson.Safe.Util in
  {
    algorithm = json |> member "algorithm" |> to_string;
    message = json |> member "message" |> to_string;
    signature = json |> member "signature" |> to_string;
    public_key = json |> member "public_key" |> to_string;
  }

let signature_verification_of_string s =
  try Ok (Yojson.Safe.from_string s |> signature_verification_of_json)
  with e -> Error (Printexc.to_string e)
