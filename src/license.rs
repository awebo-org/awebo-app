use std::path::PathBuf;
use std::time::SystemTime;

use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LICENSE_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseData {
    pub key: String,
    pub email: String,
    pub activated_at: String,
    pub machine_id: String,
    pub max_devices: u32,
    #[serde(default)]
    pub instance_id: String,
}

#[derive(Debug, Clone)]
pub enum LicenseStatus {
    Free,
    Pro(LicenseData),
}

impl LicenseStatus {
    pub fn is_pro(&self) -> bool {
        matches!(self, LicenseStatus::Pro(_))
    }
}

pub struct LicenseManager {
    status: LicenseStatus,
}

impl LicenseManager {
    pub fn load() -> Self {
        let status = match load_encrypted_license() {
            Some(data) => LicenseStatus::Pro(data),
            None => LicenseStatus::Free,
        };
        Self { status }
    }

    pub fn status(&self) -> &LicenseStatus {
        &self.status
    }

    pub fn is_pro(&self) -> bool {
        self.status.is_pro()
    }

    pub fn activate(&mut self, key: &str) -> Result<LicenseData, ActivationError> {
        let machine_id = machine_id();
        let data = validate_key_online(key, &machine_id)?;
        save_encrypted_license(&data)?;
        self.status = LicenseStatus::Pro(data.clone());
        Ok(data)
    }

    pub fn deactivate(&mut self) -> Result<(), String> {
        if let LicenseStatus::Pro(ref data) = self.status {
            let inst_id = if data.instance_id.is_empty() {
                match validate_key_online(&data.key, &data.machine_id) {
                    Ok(refreshed) => refreshed.instance_id,
                    Err(e) => return Err(format!("Could not refresh instance: {e}")),
                }
            } else {
                data.instance_id.clone()
            };

            if !inst_id.is_empty() {
                deactivate_online(&data.key, &inst_id)?;
            }
        }
        let path = license_path();
        let _ = std::fs::remove_file(&path);
        self.status = LicenseStatus::Free;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ActivationError {
    InvalidKey,
    MaxDevicesReached { used: u32, max: u32 },
    NetworkError(String),
    StorageError(String),
}

impl std::fmt::Display for ActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivationError::InvalidKey => write!(f, "Invalid license key."),
            ActivationError::MaxDevicesReached { used, max } => {
                write!(
                    f,
                    "Key already activated on maximum number of devices ({used}/{max})."
                )
            }
            ActivationError::NetworkError(e) => write!(f, "Network error: {e}"),
            ActivationError::StorageError(e) => write!(f, "Failed to save license: {e}"),
        }
    }
}

fn validate_key_online(key: &str, machine_id: &str) -> Result<LicenseData, ActivationError> {
    let body = serde_json::json!({
        "license_key": key,
        "instance_name": machine_id,
    });

    let body_str = body.to_string();
    let resp = ureq::post("https://api.lemonsqueezy.com/v1/licenses/activate")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send(body_str.as_bytes());

    match resp {
        Ok(resp) => {
            let text = resp
                .into_body()
                .read_to_string()
                .map_err(|e| ActivationError::NetworkError(e.to_string()))?;
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| ActivationError::NetworkError(e.to_string()))?;

            if json["activated"].as_bool() == Some(true) || json["valid"].as_bool() == Some(true) {
                let meta = &json["meta"];
                let email = meta["customer_email"].as_str().unwrap_or("").to_string();
                let max_devices = json["license_key"]["activation_limit"]
                    .as_u64()
                    .unwrap_or(3) as u32;
                let instance_id = json["instance"]["id"].as_str().unwrap_or("").to_string();

                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                Ok(LicenseData {
                    key: key.to_string(),
                    email,
                    activated_at: format!("{now}"),
                    machine_id: machine_id.to_string(),
                    max_devices,
                    instance_id,
                })
            } else if json["error"].as_str().is_some() {
                let msg = json["error"].as_str().unwrap_or("Unknown error");
                if msg.contains("limit") || msg.contains("activation") {
                    let used = json["license_key"]["activation_usage"]
                        .as_u64()
                        .unwrap_or(0) as u32;
                    let max = json["license_key"]["activation_limit"]
                        .as_u64()
                        .unwrap_or(3) as u32;
                    Err(ActivationError::MaxDevicesReached { used, max })
                } else {
                    Err(ActivationError::InvalidKey)
                }
            } else {
                Err(ActivationError::InvalidKey)
            }
        }
        Err(e) => Err(ActivationError::NetworkError(e.to_string())),
    }
}

fn deactivate_online(key: &str, instance_id: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "license_key": key,
        "instance_id": instance_id,
    });

    let body_str = body.to_string();
    let resp = ureq::post("https://api.lemonsqueezy.com/v1/licenses/deactivate")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send(body_str.as_bytes());

    match resp {
        Ok(resp) => {
            let text = resp.into_body().read_to_string().unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            if json["deactivated"].as_bool() == Some(true) {
                Ok(())
            } else {
                let msg = json["error"]
                    .as_str()
                    .unwrap_or("Deactivation rejected by server");
                Err(msg.to_string())
            }
        }
        Err(e) => {
            let msg = e.to_string();
            if let ureq::Error::StatusCode(code) = &e {
                Err(format!("Server returned {code}: {msg}"))
            } else {
                Err(msg)
            }
        }
    }
}

fn license_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("awebo").join("license.enc")
}

pub fn machine_id() -> String {
    let hostname = sysinfo::System::host_name().unwrap_or_default();
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let input = format!("awebo-{hostname}-{username}");
    let hash = Sha256::digest(input.as_bytes());
    hex_encode(&hash[..16])
}

fn derive_key() -> [u8; 32] {
    let mid = machine_id();
    let salt = b"awebo-license-v1";
    let input = format!("{mid}-{}", String::from_utf8_lossy(salt));
    let hash = Sha256::digest(input.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

fn save_encrypted_license(data: &LicenseData) -> Result<(), ActivationError> {
    let json =
        serde_json::to_string(data).map_err(|e| ActivationError::StorageError(e.to_string()))?;

    let key = derive_key();
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|e| ActivationError::StorageError(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    chacha20poly1305::aead::rand_core::RngCore::fill_bytes(&mut OsRng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| ActivationError::StorageError(e.to_string()))?;

    let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    out.push(LICENSE_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    let path = license_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ActivationError::StorageError(e.to_string()))?;
    }
    std::fs::write(&path, &out).map_err(|e| ActivationError::StorageError(e.to_string()))?;

    Ok(())
}

fn load_encrypted_license() -> Option<LicenseData> {
    let path = license_path();
    let bytes = std::fs::read(&path).ok()?;

    if bytes.len() < 1 + NONCE_LEN + 16 {
        return None;
    }

    let version = bytes[0];
    if version != LICENSE_VERSION {
        return None;
    }

    let nonce_bytes = &bytes[1..1 + NONCE_LEN];
    let ciphertext = &bytes[1 + NONCE_LEN..];

    let key = derive_key();
    let cipher = ChaCha20Poly1305::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    let json = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json).ok()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_id_is_deterministic() {
        let id1 = machine_id();
        let id2 = machine_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 32);
    }

    #[test]
    fn derive_key_is_deterministic() {
        let k1 = derive_key();
        let k2 = derive_key();
        assert_eq!(k1, k2);
    }

    #[test]
    fn hex_encode_works() {
        assert_eq!(hex_encode(&[0xff, 0x00, 0xab]), "ff00ab");
    }

    #[test]
    fn activation_error_display() {
        let e = ActivationError::InvalidKey;
        assert_eq!(e.to_string(), "Invalid license key.");
    }
}
