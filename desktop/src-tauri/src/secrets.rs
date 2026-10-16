use std::collections::BTreeMap;
use std::fs;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::paths;

const FORMAT: &str = "speech-to-text-sealed-v1";
const KEY_INFO: &[u8] = b"com.jdrechsler.SpeechToText api keys";
const NONCE_LENGTH: usize = 12;

#[derive(Serialize, Deserialize)]
struct SealedFile {
    format: String,
    salt: String,
    sealed: String,
}

pub fn read(name: &str) -> Option<String> {
    load_all().remove(name).filter(|value| !value.is_empty())
}

pub fn contains(name: &str) -> bool {
    read(name).is_some()
}

pub fn save(name: &str, value: &str) -> Result<(), String> {
    let mut all = load_all();
    all.insert(name.to_string(), value.to_string());
    write(&all)
}

pub fn delete(name: &str) -> Result<(), String> {
    let mut all = load_all();
    if all.remove(name).is_none() {
        return Ok(());
    }
    write(&all)
}

pub fn sealed(values: &BTreeMap<String, String>, machine_id: &str) -> Option<Vec<u8>> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut nonce = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut nonce);

    let cipher = Aes256Gcm::new_from_slice(&derived_key(machine_id, &salt)).ok()?;
    let plain = serde_json::to_vec(values).ok()?;
    let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), plain.as_slice()).ok()?;

    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    let file = SealedFile {
        format: FORMAT.into(),
        salt: BASE64.encode(salt),
        sealed: BASE64.encode(combined),
    };
    serde_json::to_vec(&file).ok()
}

pub fn opened(data: &[u8], machine_id: &str) -> Option<BTreeMap<String, String>> {
    let file: SealedFile = serde_json::from_slice(data).ok()?;
    if file.format != FORMAT {
        return None;
    }
    let salt = BASE64.decode(file.salt).ok()?;
    let combined = BASE64.decode(file.sealed).ok()?;
    if combined.len() <= NONCE_LENGTH {
        return None;
    }
    let (nonce, ciphertext) = combined.split_at(NONCE_LENGTH);
    let cipher = Aes256Gcm::new_from_slice(&derived_key(machine_id, &salt)).ok()?;
    let plain = cipher.decrypt(Nonce::from_slice(nonce), ciphertext).ok()?;
    serde_json::from_slice(&plain).ok()
}

fn load_all() -> BTreeMap<String, String> {
    let (Some(machine_id), Ok(data)) = (machine_id(), fs::read(paths::keys_file())) else {
        return BTreeMap::new();
    };
    opened(&data, &machine_id).unwrap_or_default()
}

fn write(values: &BTreeMap<String, String>) -> Result<(), String> {
    let machine_id = machine_id().ok_or("Could not read this computer's ID")?;
    let data = sealed(values, &machine_id).ok_or("Could not encrypt the keys")?;
    let path = paths::keys_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("enc.tmp");
    fs::write(&temporary, data).map_err(|error| error.to_string())?;
    restrict_to_owner(&temporary);
    fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn restrict_to_owner(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &std::path::Path) {}

fn derived_key(machine_id: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(salt), machine_id.as_bytes())
        .expand(KEY_INFO, &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

fn machine_id() -> Option<String> {
    machine_uid::get().ok().filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_keys_open_only_on_the_same_machine() {
        let keys = BTreeMap::from([("ELEVENLABS_API_KEY".to_string(), "sk-test-123".to_string())]);
        let data = sealed(&keys, "machine-a").unwrap();
        assert_eq!(opened(&data, "machine-a"), Some(keys));
        assert_eq!(opened(&data, "machine-b"), None);
        assert!(!String::from_utf8_lossy(&data).contains("sk-test"));
    }
}
