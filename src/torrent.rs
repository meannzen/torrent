use serde::{de::Visitor, Deserialize, Serialize};
use sha1::{Digest, Sha1};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

impl Torrent {
    pub fn info_hash(&self) -> [u8; 20] {
        let info_encoded =
            serde_bencode::to_bytes(&self.info).expect("re-encode info section should be fine");
        let mut hasher = Sha1::new();
        hasher.update(&info_encoded);
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct Info {
    pub name: String,
    #[serde(rename = "piece length")]
    pub plength: usize,
    pub pieces: Hashes,
    #[serde(flatten)]
    pub keys: Keys,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Keys {
    SingleFile { length: usize },
    MultiFile { file: Vec<File> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct File {
    pub length: usize,
    pub path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Hashes(pub Vec<[u8; 20]>);
struct HashesVisitor;

impl<'d> Visitor<'d> for HashesVisitor {
    type Value = Hashes;
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.len() % 20 != 0 {
            return Err(E::custom(format!("length is {}", v.len())));
        }
        Ok(Hashes(
            v.chunks_exact(20)
                .map(|s| s.try_into().expect("to be length 20"))
                .collect(),
        ))
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a byte string length is a mutiple of 20")
    }
}

impl<'de> Deserialize<'de> for Hashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(HashesVisitor)
    }
}

impl Serialize for Hashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let signle_slice = self.0.concat();
        serializer.serialize_bytes(&signle_slice)
    }
}
