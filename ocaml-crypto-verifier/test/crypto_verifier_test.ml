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

let test_ed25519_not_implemented (_ctx : test_ctxt) =
  let sv : signature_verification = {
    algorithm = "ed25519";
    message = "Hello, World!";
    signature = Base64.encode_string "dummy_signature";
    public_key = Base64.encode_string (String.make 32 '\x00');
  } in
  let result = verify_signature sv in
  assert_bool "Ed25519 not implemented: should return valid=false" (not result.valid)

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
         "test_ed25519_not_implemented" >:: test_ed25519_not_implemented;
         "test_json_parse_encryption" >:: test_json_parse_encryption;
         "test_json_parse_invalid" >:: test_json_parse_invalid;
       ]

let () = run_test_tt_main suite
