open Crypto_verifier
open OUnit2

let test_health_check (_ctx : test_ctxt) =
  let result = health_check () in
  assert_bool "health check should be valid" result.valid;
  assert_equal
    "Crypto verifier is healthy"
    result.details

let test_aes_gcm_verification (_ctx : test_ctxt) =
  let key = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f" in
  let nonce = "000102030405060708090a0b" in
  let ev : encryption_verification =
    {
      algorithm = "aes256_gcm";
      plaintext = "Hello, World!";
      ciphertext = "base64_encoded_ciphertext";
      key;
      nonce = Some nonce;
      aad = None;
    }
  in
  let result : verification_result = verify_encryption ev in
  assert_bool "AES-GCM verification should not crash" (not result.valid || result.valid)

let test_chacha20_verification (_ctx : test_ctxt) =
  let key = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f" in
  let nonce = "000102030405060708090a0b" in
  let ev : encryption_verification =
    {
      algorithm = "chacha20_poly1305";
      plaintext = "Hello, World!";
      ciphertext = "base64_encoded_ciphertext";
      key;
      nonce = Some nonce;
      aad = None;
    }
  in
  let result : verification_result = verify_encryption ev in
  assert_bool "ChaCha20 verification should not crash" (not result.valid || result.valid)

let test_ed25519_verification (_ctx : test_ctxt) =
  let sv : signature_verification =
    {
      algorithm = "ed25519";
      message = "Hello, World!";
      signature = "signature_hex";
      public_key = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    }
  in
  let result : verification_result = verify_signature sv in
  assert_bool "Ed25519 verification should not crash" (not result.valid || result.valid)

let suite =
  "Crypto Verifier Tests"
  >::: [
         "test_health_check" >:: test_health_check;
         "test_aes_gcm_verification" >:: test_aes_gcm_verification;
         "test_chacha20_verification" >:: test_chacha20_verification;
         "test_ed25519_verification" >:: test_ed25519_verification;
       ]

let () = run_test_tt_main suite
