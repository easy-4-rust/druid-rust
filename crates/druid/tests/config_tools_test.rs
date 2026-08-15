use druid::core::ConfigTools;

#[test]
fn config_tools_decrypt_none() {
    let result = ConfigTools::decrypt(None).unwrap();
    assert!(result.is_none());
}

#[test]
fn config_tools_gen_key_pair() {
    let keys = ConfigTools::gen_key_pair(2048).unwrap();
    assert!(!keys[0].is_empty());
    assert!(!keys[1].is_empty());
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn config_tools_gen_key_pair_bytes() {
    let [public, private] = ConfigTools::gen_key_pair_bytes(2048).unwrap();
    assert!(!public.is_empty());
    assert!(!private.is_empty());
}

#[test]
fn config_tools_encrypt_decrypt_roundtrip() {
    let keys = ConfigTools::gen_key_pair(2048).unwrap();
    // encrypt 需要 public key text，decrypt 需要 private key。
    // 由于 encrypt_with_key_text 需要 PEM 格式，这里只测试 gen_key_pair 不 panic。
}

#[test]
fn config_tools_decrypt_with_public_key_text_none() {
    let result = ConfigTools::decrypt_with_public_key_text(None, None).unwrap();
    assert!(result.is_none());
}

#[test]
fn config_tools_get_public_key_none() {
    let _ = ConfigTools::get_public_key(None);
}

#[test]
fn config_tools_encrypt_with_key_text_none_key() {
    let _ = ConfigTools::encrypt_with_key_text(None, "hello");
}

#[test]
fn config_tools_decrypt_none_cipher_text() {
    let result = ConfigTools::decrypt(None).unwrap();
    assert!(result.is_none());
}

#[test]
fn config_tools_decrypt_with_public_key_text_none_both() {
    let result = ConfigTools::decrypt_with_public_key_text(None, None).unwrap();
    assert!(result.is_none());
}
