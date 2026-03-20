open Crypto_verifier
open OUnit2

let test_health_check (_ctx : test_ctxt) =
  let result = health_check () in
  assert_bool "health check should be valid" result.valid;
  assert_equal "Crypto verifier is healthy" result.details

(* Self-consistency test: encrypt with mirage-crypto, then verify with our library *)
let test_aes_gcm_valid (_ctx : test_ctxt) =
  let passphrase = "test_passphrase" in
  let salt_str = String.make 16 '\x00' in
  let nonce_str = String.make 12 '\x01' in
  let plaintext = "Hello, AES-GCM!" in
  let salt_cs = Cstruct.of_string salt_str in
  let key_material = Mirage_crypto.Hash.SHA256.digest
    (Cstruct.concat [salt_cs; Cstruct.of_string passphrase]) in
  let key = Mirage_crypto.Cipher_block.AES.GCM.of_secret key_material in
  let nonce_cs = Cstruct.of_string nonce_str in
  let ct_cs = Mirage_crypto.Cipher_block.AES.GCM.authenticate_encrypt
    ~key ~nonce:nonce_cs ~adata:Cstruct.empty (Cstruct.of_string plaintext) in
  let ev : encryption_verification = {
    algorithm = "aes256_gcm";
    plaintext;
    ciphertext = Base64.encode_string (Cstruct.to_string ct_cs);
    key = passphrase;
    nonce = Some (Base64.encode_string nonce_str);
    salt = Some (Base64.encode_string salt_str);
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "AES-GCM valid ciphertext should verify" result.valid

let test_aes_gcm_tampered (_ctx : test_ctxt) =
  let ev : encryption_verification = {
    algorithm = "aes256_gcm";
    plaintext = "hello";
    ciphertext = Base64.encode_string (String.make 32 '\xff');
    key = "passphrase";
    nonce = Some (Base64.encode_string (String.make 12 '\x00'));
    salt = Some (Base64.encode_string (String.make 16 '\x00'));
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "Tampered AES-GCM ciphertext should fail" (not result.valid)

let test_aes_gcm_missing_nonce (_ctx : test_ctxt) =
  let ev : encryption_verification = {
    algorithm = "aes256_gcm";
    plaintext = "hello";
    ciphertext = Base64.encode_string "dummy";
    key = "key";
    nonce = None;
    salt = Some (Base64.encode_string (String.make 16 '\x00'));
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "Missing nonce should fail" (not result.valid)

let test_aes_gcm_missing_salt (_ctx : test_ctxt) =
  let ev : encryption_verification = {
    algorithm = "aes256_gcm";
    plaintext = "hello";
    ciphertext = Base64.encode_string "dummy";
    key = "key";
    nonce = Some (Base64.encode_string (String.make 12 '\x00'));
    salt = None;
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "Missing salt should fail" (not result.valid)

(* Self-consistency test: encrypt with mirage-crypto ChaCha20, then verify *)
let test_chacha20_valid (_ctx : test_ctxt) =
  let passphrase = "test_passphrase" in
  let salt_str = String.make 16 '\x00' in
  let nonce_str = String.make 12 '\x02' in
  let plaintext = "Hello, ChaCha20!" in
  let salt_cs = Cstruct.of_string salt_str in
  let key_material = Mirage_crypto.Hash.SHA256.digest
    (Cstruct.concat [salt_cs; Cstruct.of_string passphrase]) in
  let nonce_cs = Cstruct.of_string nonce_str in
  let ct_cs = Mirage_crypto.Chacha20.authenticate_encrypt
    ~key:key_material ~nonce:nonce_cs (Cstruct.of_string plaintext) in
  let ev : encryption_verification = {
    algorithm = "chacha20_poly1305";
    plaintext;
    ciphertext = Base64.encode_string (Cstruct.to_string ct_cs);
    key = passphrase;
    nonce = Some (Base64.encode_string nonce_str);
    salt = Some (Base64.encode_string salt_str);
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "ChaCha20-Poly1305 valid ciphertext should verify" result.valid

let test_chacha20_tampered (_ctx : test_ctxt) =
  let ev : encryption_verification = {
    algorithm = "chacha20_poly1305";
    plaintext = "hello";
    ciphertext = Base64.encode_string (String.make 32 '\xff');
    key = "passphrase";
    nonce = Some (Base64.encode_string (String.make 12 '\x00'));
    salt = Some (Base64.encode_string (String.make 16 '\x00'));
    aad = None;
  } in
  let result = verify_encryption ev in
  assert_bool "Tampered ChaCha20 ciphertext should fail" (not result.valid)

(* Invalid signature + zero public key must be rejected *)
let test_ed25519_invalid_signature_fails (_ctx : test_ctxt) =
  let sv : signature_verification = {
    algorithm = "ed25519";
    message = "Hello, World!";
    signature = Base64.encode_string (String.make 64 '\x00');
    public_key = Base64.encode_string (String.make 32 '\x00');
  } in
  let result = verify_signature sv in
  assert_bool "Ed25519 invalid signature should return valid=false" (not result.valid)

(* RFC 8032 Section 6 Test Vector 2: single-byte message *)
let test_ed25519_rfc8032_vector2 (_ctx : test_ctxt) =
  (* Public key (32 bytes) *)
  let pub_bytes =
    "\x3d\x40\x17\xc3\xe8\x43\x89\x5a\x92\xb7\x0a\xa7\x4d\x1b\x7e\xbc" ^
    "\x9c\x98\x2c\xcf\x2e\xc4\x96\x8c\xc0\xcd\x55\xf1\x2a\xf4\x66\x0c"
  in
  (* Message: single byte 0x72 *)
  let msg = "\x72" in
  (* Signature (64 bytes) *)
  let sig_bytes =
    "\x92\xa0\x09\xa9\xf0\xd4\xca\xb8\x72\x0e\x82\x0b\x5f\x64\x25\x40" ^
    "\xa2\xb2\x7b\x54\x16\x50\x3f\x8f\xb3\x76\x22\x23\xeb\xdb\x69\xda" ^
    "\x08\x5a\xc1\xe4\x3e\x15\x99\x6e\x45\x8f\x36\x13\xd0\xf1\x1d\x8c" ^
    "\x38\x7b\x2e\xae\xb4\x30\x2a\xee\xb0\x0d\x29\x16\x12\xbb\x0c\x00"
  in
  let sv : signature_verification = {
    algorithm = "ed25519";
    message = msg;
    signature = Base64.encode_string sig_bytes;
    public_key = Base64.encode_string pub_bytes;
  } in
  let result = verify_signature sv in
  assert_bool "RFC 8032 Test Vector 2: valid Ed25519 signature should verify" result.valid

let test_json_parse_encryption (_ctx : test_ctxt) =
  let json_str = {|{"algorithm":"aes256_gcm","plaintext":"hello","ciphertext":"dGVzdA==","key":"pass","nonce":"AAAA","salt":"AAAA","aad":null}|} in
  (match encryption_verification_of_string json_str with
  | Ok ev ->
    assert_equal "aes256_gcm" ev.algorithm;
    assert_equal "hello" ev.plaintext;
    assert_equal "pass" ev.key;
    assert_equal (Some "AAAA") ev.nonce;
    assert_equal (Some "AAAA") ev.salt
  | Error msg -> assert_failure ("JSON parse failed: " ^ msg))

let test_json_parse_invalid (_ctx : test_ctxt) =
  let result = encryption_verification_of_string "not valid json" in
  assert_bool "Invalid JSON should return Error" (match result with Error _ -> true | Ok _ -> false)

let suite =
  "Crypto Verifier Tests"
  >::: [
         "test_health_check" >:: test_health_check;
         "test_aes_gcm_valid" >:: test_aes_gcm_valid;
         "test_aes_gcm_tampered" >:: test_aes_gcm_tampered;
         "test_aes_gcm_missing_nonce" >:: test_aes_gcm_missing_nonce;
         "test_aes_gcm_missing_salt" >:: test_aes_gcm_missing_salt;
         "test_chacha20_valid" >:: test_chacha20_valid;
         "test_chacha20_tampered" >:: test_chacha20_tampered;
         "test_ed25519_invalid_signature_fails" >:: test_ed25519_invalid_signature_fails;
         "test_ed25519_rfc8032_vector2" >:: test_ed25519_rfc8032_vector2;
         "test_json_parse_encryption" >:: test_json_parse_encryption;
         "test_json_parse_invalid" >:: test_json_parse_invalid;
       ]

let () = run_test_tt_main suite
