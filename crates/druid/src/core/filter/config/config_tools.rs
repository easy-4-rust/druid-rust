//! 对应 Java：`com.alibaba.druid.filter.config.ConfigTools`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/config/ConfigTools.java`。

use crate::core::DruidError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use rsa::rand_core::OsRng;
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rsa::{BigUint, RsaPrivateKey, RsaPublicKey};
use std::fs;
use std::path::Path;
use x509_cert::der::{Decode, Encode};
use x509_cert::Certificate;

/// Druid 旧版配置密文兼容工具。
///
/// Java `ConfigTools` 使用非典型的“私钥 PKCS#1 v1.5 type-1 运算、公钥恢复”
/// 来隐藏数据源密码。该算法不应作为新协议使用，但迁移必须能读取现有 Druid
/// 密文，因此这里显式实现其编码块并保留 PKCS#8 私钥、X.509 `SubjectPublicKeyInfo`
/// 公钥、标准 Base64 以及默认 512-bit 历史密钥格式。
#[derive(Debug, Default, Clone, Copy)]
#[deprecated(note = "仅用于读取 Java Druid 旧配置密文；新系统应使用 secret manager")]
pub struct ConfigTools;

#[allow(deprecated)]
impl ConfigTools {
    /// Java Druid 1.2.28 的默认 X.509/SPKI RSA 公钥。
    pub const DEFAULT_PUBLIC_KEY_STRING: &'static str =
        "MFwwDQYJKoZIhvcNAQEBBQADSwAwSAJBAKHGwq7q2RmwuRgKxBypQHw0mYu4BQZ3eMsTrdK8E6igRcxsobUC7uT0SoxIjl1WveWniCASejoQtn/BY6hVKWsCAwEAAQ==";

    const DEFAULT_PRIVATE_KEY_STRING: &'static str =
        "MIIBVAIBADANBgkqhkiG9w0BAQEFAASCAT4wggE6AgEAAkEAocbCrurZGbC5GArEHKlAfDSZi7gFBnd4yxOt0rwTqKBFzGyhtQLu5PRKjEiOXVa95aeIIBJ6OhC2f8FjqFUpawIDAQABAkAPejKaBYHrwUqUEEOe8lpnB6lBAsQIUFnQI/vXU4MV+MhIzW0BLVZCiarIQqUXeOhThVWXKFt8GxCykrrUsQ6BAiEA4vMVxEHBovz1di3aozzFvSMdsjTcYRRo82hS5Ru2/OECIQC2fAPoXixVTVY7bNMeuxCP4954ZkXp7fEPDINCjcQDywIgcc8XLkkPcs3Jxk7uYofaXaPbg39wuJpEmzPIxi3k0OECIGubmdpOnin3HuCP/bbjbJLNNoUdGiEmFL5hDI4UdwAdAiEAtcAwbm08bKN7pwwvyqaCBC//VnEWaq39DCzxr+Z2EIk=";

    /// 使用默认公钥恢复可空密文。
    ///
    /// `None` 和空字符串与 Java 一样原样返回；公钥仍会先被解析。
    pub fn decrypt(cipher_text: Option<&str>) -> Result<Option<String>, DruidError> {
        Self::decrypt_with_public_key_text(None, cipher_text)
    }

    /// 使用可选 Base64 X.509/SPKI 公钥恢复可空密文。
    pub fn decrypt_with_public_key_text(
        public_key_text: Option<&str>,
        cipher_text: Option<&str>,
    ) -> Result<Option<String>, DruidError> {
        let public_key = Self::get_public_key(public_key_text)?;
        Self::decrypt_with_public_key(&public_key, cipher_text)
    }

    /// 使用已解析 RSA 公钥恢复可空密文。
    pub fn decrypt_with_public_key(
        public_key: &RsaPublicKey,
        cipher_text: Option<&str>,
    ) -> Result<Option<String>, DruidError> {
        let Some(cipher_text) = cipher_text else {
            return Ok(None);
        };
        if cipher_text.is_empty() {
            return Ok(Some(String::new()));
        }
        let cipher_bytes = STANDARD.decode(cipher_text).map_err(|error| {
            DruidError::InvalidArgument(format!("invalid config ciphertext base64: {error}"))
        })?;
        let plain_bytes = public_key_type1_decrypt(public_key, &cipher_bytes)?;
        Ok(Some(String::from_utf8_lossy(&plain_bytes).into_owned()))
    }

    /// 解析可选 Base64 X.509/SPKI 公钥；空值使用 Java 默认公钥。
    pub fn get_public_key(public_key_text: Option<&str>) -> Result<RsaPublicKey, DruidError> {
        let public_key_text = public_key_text
            .filter(|text| !text.is_empty())
            .unwrap_or(Self::DEFAULT_PUBLIC_KEY_STRING);
        let key_bytes = STANDARD.decode(public_key_text).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })?;
        RsaPublicKey::from_public_key_der(&key_bytes).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })
    }

    /// 从 DER X.509 证书读取 RSA 公钥；空路径使用默认公钥。
    pub fn get_public_key_by_x509(x509_file: Option<&Path>) -> Result<RsaPublicKey, DruidError> {
        let Some(x509_file) = x509_file.filter(|path| !path.as_os_str().is_empty()) else {
            return Self::get_public_key(None);
        };
        let bytes = fs::read(x509_file).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })?;
        let certificate = Certificate::from_der(&bytes).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })?;
        let spki = certificate
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .map_err(|error| {
                DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
            })?;
        RsaPublicKey::from_public_key_der(&spki).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })
    }

    /// 从 DER X.509/SPKI 公钥文件读取 RSA 公钥；空路径使用默认公钥。
    pub fn get_public_key_by_public_key_file(
        public_key_file: Option<&Path>,
    ) -> Result<RsaPublicKey, DruidError> {
        let Some(public_key_file) = public_key_file.filter(|path| !path.as_os_str().is_empty())
        else {
            return Self::get_public_key(None);
        };
        let bytes = fs::read(public_key_file).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })?;
        RsaPublicKey::from_public_key_der(&bytes).map_err(|error| {
            DruidError::InvalidArgument(format!("Failed to get public key: {error}"))
        })
    }

    /// 使用默认 PKCS#8 私钥产生 Java 兼容密文。
    pub fn encrypt(plain_text: &str) -> Result<String, DruidError> {
        Self::encrypt_with_key_text(None, plain_text)
    }

    /// 使用可选 Base64 PKCS#8 私钥产生 Java 兼容密文。
    pub fn encrypt_with_key_text(
        private_key_text: Option<&str>,
        plain_text: &str,
    ) -> Result<String, DruidError> {
        let private_key_text = private_key_text.unwrap_or(Self::DEFAULT_PRIVATE_KEY_STRING);
        let key_bytes = STANDARD.decode(private_key_text).map_err(|error| {
            DruidError::InvalidArgument(format!("invalid PKCS#8 private key base64: {error}"))
        })?;
        Self::encrypt_with_key_bytes(&key_bytes, plain_text)
    }

    /// 使用 DER PKCS#8 私钥产生 Java 兼容密文。
    pub fn encrypt_with_key_bytes(
        key_bytes: &[u8],
        plain_text: &str,
    ) -> Result<String, DruidError> {
        let private_key = RsaPrivateKey::from_pkcs8_der(key_bytes).map_err(|error| {
            DruidError::InvalidArgument(format!("invalid PKCS#8 private key: {error}"))
        })?;
        let encrypted = private_key_type1_encrypt(&private_key, plain_text.as_bytes())?;
        Ok(STANDARD.encode(encrypted))
    }

    /// 生成 `[PKCS#8 private, X.509/SPKI public]` DER 密钥对。
    pub fn gen_key_pair_bytes(key_size: usize) -> Result<[Vec<u8>; 2], DruidError> {
        let private_key = RsaPrivateKey::new(&mut OsRng, key_size).map_err(|error| {
            DruidError::InvalidArgument(format!("failed to generate RSA key pair: {error}"))
        })?;
        let public_key = RsaPublicKey::from(&private_key);
        let private_der = private_key.to_pkcs8_der().map_err(|error| {
            DruidError::InvalidArgument(format!("failed to encode private key: {error}"))
        })?;
        let public_der = public_key.to_public_key_der().map_err(|error| {
            DruidError::InvalidArgument(format!("failed to encode public key: {error}"))
        })?;
        Ok([
            private_der.as_bytes().to_vec(),
            public_der.as_bytes().to_vec(),
        ])
    }

    /// 生成 `[Base64 PKCS#8 private, Base64 X.509/SPKI public]` 密钥对。
    pub fn gen_key_pair(key_size: usize) -> Result<[String; 2], DruidError> {
        let [private_key, public_key] = Self::gen_key_pair_bytes(key_size)?;
        Ok([STANDARD.encode(private_key), STANDARD.encode(public_key)])
    }
}

fn private_key_type1_encrypt(
    private_key: &RsaPrivateKey,
    message: &[u8],
) -> Result<Vec<u8>, DruidError> {
    let modulus_len = private_key.size();
    if message.len() > modulus_len.saturating_sub(11) {
        return Err(DruidError::InvalidArgument(format!(
            "RSA plaintext is too long: {} bytes for {}-byte modulus",
            message.len(),
            modulus_len
        )));
    }

    let padding_len = modulus_len - message.len() - 3;
    let mut encoded = Vec::with_capacity(modulus_len);
    encoded.extend_from_slice(&[0, 1]);
    encoded.extend(std::iter::repeat_n(0xff, padding_len));
    encoded.push(0);
    encoded.extend_from_slice(message);

    let message_integer = BigUint::from_bytes_be(&encoded);
    let cipher_integer = message_integer.modpow(private_key.d(), private_key.n());
    left_pad(cipher_integer.to_bytes_be(), modulus_len)
}

fn public_key_type1_decrypt(
    public_key: &RsaPublicKey,
    cipher_text: &[u8],
) -> Result<Vec<u8>, DruidError> {
    let modulus_len = public_key.size();
    if cipher_text.len() != modulus_len {
        return Err(DruidError::InvalidArgument(format!(
            "RSA ciphertext length must be {modulus_len}, actual={}",
            cipher_text.len()
        )));
    }
    let cipher_integer = BigUint::from_bytes_be(cipher_text);
    if &cipher_integer >= public_key.n() {
        return Err(DruidError::InvalidArgument(
            "RSA ciphertext representative out of range".to_owned(),
        ));
    }
    let message_integer = cipher_integer.modpow(public_key.e(), public_key.n());
    let encoded = left_pad(message_integer.to_bytes_be(), modulus_len)?;

    if encoded.len() < 11 || encoded[0] != 0 || encoded[1] != 1 {
        return Err(DruidError::InvalidArgument(
            "invalid RSA PKCS#1 v1.5 type-1 block".to_owned(),
        ));
    }
    let delimiter = encoded[2..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|position| position + 2)
        .ok_or_else(|| {
            DruidError::InvalidArgument("invalid RSA PKCS#1 v1.5 type-1 delimiter".to_owned())
        })?;
    if delimiter < 10 || encoded[2..delimiter].iter().any(|byte| *byte != 0xff) {
        return Err(DruidError::InvalidArgument(
            "invalid RSA PKCS#1 v1.5 type-1 padding".to_owned(),
        ));
    }
    Ok(encoded[delimiter + 1..].to_vec())
}

fn left_pad(mut bytes: Vec<u8>, len: usize) -> Result<Vec<u8>, DruidError> {
    if bytes.len() > len {
        return Err(DruidError::InvalidArgument(
            "RSA representative exceeds modulus length".to_owned(),
        ));
    }
    if bytes.len() == len {
        return Ok(bytes);
    }
    let mut padded = vec![0; len - bytes.len()];
    padded.append(&mut bytes);
    Ok(padded)
}
