open Mirage_crypto
open Mirage_crypto_rng
open Mirage_crypto_ec
module AEAD = Mirage_crypto.AES.GCM
module ChaCha = Mirage_crypto.ChaCha20
module Poly1305 = Mirage_crypto.Poly1305
module Hash = Mirage_crypto.Hash

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

let hex_to_bytes hex =
  match Hex.of_string hex with
  | `Hex s -> Cstruct.of_string s
  | `Invalid_hex_char c -> raise (Verification_error (Printf.sprintf "Invalid hex character: %c" c))

let bytes_to_hex cs = Hex.show (Hex.of_cstruct cs)

let base64_to_bytes b64 =
  match Base64.decode ~pad:false b64 with
  | Ok s -> Cstruct.of_string s
  | Error (`Msg msg) -> raise (Verification_error ("Base64 decode error: " ^ msg))

let bytes_to_base64 cs = Base64.encode_string ~pad:false (Cstruct.to_string cs)

let verify_aes_gcm ev =
  try
    let key = hex_to_bytes ev.key in
    let plaintext = Cstruct.of_string ev.plaintext in
    let ciphertext = base64_to_bytes ev.ciphertext in
    let nonce = Option.map hex_to_bytes ev.nonce |> Option.value ~default:(Cstruct.create 12) in
    let aad = Option.map Cstruct.of_string ev.aad |> Option.value ~default:Cstruct.empty in

    match AEAD.GCM.authenticate_encrypt ~key ~nonce ~adata:aad plaintext with
    | Ok computed_ciphertext when Cstruct.equal computed_ciphertext ciphertext ->
        { valid = true; details = "AES-GCM verification successful"; timestamp = now () }
    | Ok _ ->
        { valid = false; details = "AES-GCM ciphertext mismatch"; timestamp = now () }
    | Error `Authentication_failure ->
        { valid = false; details = "AES-GCM authentication failed"; timestamp = now () }
    | Error (`Invalid_length msg) ->
        { valid = false; details = "AES-GCM invalid length: " ^ msg; timestamp = now () }
  with
  | Verification_error msg -> { valid = false; details = msg; timestamp = now () }
  | exn -> { valid = false; details = "AES-GCM verification error: " ^ Printexc.to_string exn; timestamp = now () }

let verify_chacha20_poly1305 ev =
  try
    let key = hex_to_bytes ev.key in
    let plaintext = Cstruct.of_string ev.plaintext in
    let ciphertext = base64_to_bytes ev.ciphertext in
    let nonce = Option.map hex_to_bytes ev.nonce |> Option.value ~default:(Cstruct.create 12) in
    let aad = Option.map Cstruct.of_string ev.aad |> Option.value ~default:Cstruct.empty in

    match ChaCha20.Poly1305.authenticate_encrypt ~key ~nonce ~adata:aad plaintext with
    | Ok computed_ciphertext when Cstruct.equal computed_ciphertext ciphertext ->
        { valid = true; details = "ChaCha20-Poly1305 verification successful"; timestamp = now () }
    | Ok _ ->
        { valid = false; details = "ChaCha20-Poly1305 ciphertext mismatch"; timestamp = now () }
    | Error `Authentication_failure ->
        { valid = false; details = "ChaCha20-Poly1305 authentication failed"; timestamp = now () }
    | Error (`Invalid_length msg) ->
        { valid = false; details = "ChaCha20-Poly1305 invalid length: " ^ msg; timestamp = now () }
  with
  | Verification_error msg -> { valid = false; details = msg; timestamp = now () }
  | exn -> { valid = false; details = "ChaCha20-Poly1305 verification error: " ^ Printexc.to_string exn; timestamp = now () }

let verify_ed25519 sv =
  try
    let public_key = hex_to_bytes sv.public_key in
    let signature = hex_to_bytes sv.signature in
    let message = Cstruct.of_string sv.message in

    match Ed25519.verify public_key signature message with
    | true -> { valid = true; details = "Ed25519 signature verification successful"; timestamp = now () }
    | false -> { valid = false; details = "Ed25519 signature verification failed"; timestamp = now () }
  with
  | Verification_error msg -> { valid = false; details = msg; timestamp = now () }
  | exn -> { valid = false; details = "Ed25519 verification error: " ^ Printexc.to_string exn; timestamp = now () }

let verify_encryption ev =
  match String.lowercase_ascii ev.algorithm with
  | "aes256_gcm" | "aes-gcm" -> verify_aes_gcm ev
  | "chacha20_poly1305" | "chacha20-poly1305" -> verify_chacha20_poly1305 ev
  | "xchacha20_poly1305" | "xchacha20-poly1305" ->
      (* XChaCha20-Poly1305 would need different implementation *)
      { valid = false; details = "XChaCha20-Poly1305 not yet implemented"; timestamp = now () }
  | alg -> { valid = false; details = "Unsupported encryption algorithm: " ^ alg; timestamp = now () }

let verify_signature sv =
  match String.lowercase_ascii sv.algorithm with
  | "ed25519" -> verify_ed25519 sv
  | alg -> { valid = false; details = "Unsupported signature algorithm: " ^ alg; timestamp = now () }

(* Health check function *)
let health_check () = { valid = true; details = "Crypto verifier is healthy"; timestamp = now () }
