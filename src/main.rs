use anyhow::Context;
use clap::{Parser, Subcommand};
use core::panic;
use peers::Peers;
use serde::{de::Visitor, Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Torrent {
    announce: String,
    info: Info,
}

impl Torrent {
    fn info_hash(&self) -> [u8; 20] {
        let info_encoded =
            serde_bencode::to_bytes(&self.info).expect("re-encode info section should be fine");
        let mut hasher = Sha1::new();
        hasher.update(&info_encoded);
        hasher
            .finalize()
            .try_into()
            .expect("Generice Array<_, 20> ==[_;20]")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
struct Info {
    name: String,
    #[serde(rename = "piece length")]
    plength: usize,
    pieces: Hashes,
    length: usize,
}

#[derive(Debug, Clone)]
struct Hashes(Vec<[u8; 20]>);
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
                .map(|s| s.try_into().expect("to be length 2o"))
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

fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    let first = encoded_value.chars().next();

    match first {
        Some('l') => {
            let mut values = Vec::new();
            let mut rest = encoded_value.split_at(1).1;
            while !rest.is_empty() && !rest.starts_with('e') {
                let (value, remainder) = decode_bencoded_value(rest);
                values.push(value);
                rest = remainder;
            }

            return (values.into(), &rest[1..]);
        }
        Some('i') => {
            if let Some((number, rest)) =
                encoded_value
                    .split_at(1)
                    .1
                    .split_once('e')
                    .and_then(|(digit, rest)| {
                        let n: i64 = digit.parse().ok()?;
                        Some((n, rest))
                    })
            {
                return (number.into(), rest);
            }
        }
        Some('0'..='9') => {
            if let Some((string, rest)) = encoded_value.split_once(":").and_then(|(len, last)| {
                let len: usize = len.parse().ok()?;
                Some((last[..len].to_string(), &last[len..]))
            }) {
                return (string.into(), rest);
            }
        }

        Some('d') => {
            let mut dict = serde_json::Map::new();
            let mut rest = encoded_value.split_at(1).1;
            while !rest.is_empty() && !rest.starts_with('e') {
                let (k, remainder) = decode_bencoded_value(rest);
                let k = match k {
                    serde_json::Value::String(k) => k,
                    k => {
                        panic!("dict key must be string, not {k:?}");
                    }
                };

                let (v, remainder) = decode_bencoded_value(remainder);
                rest = remainder;
                dict.insert(k, v);
            }

            return (dict.into(), &rest[1..]);
        }
        _ => {}
    }

    panic!("Unhandled encoded value: {}", encoded_value)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Decode { value: String },
    Info { torrent: PathBuf },
    Peers { torrent: PathBuf },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Decode { value } => {
            let v = decode_bencoded_value(&value).0;
            println!("{v}");
        }

        Command::Info { torrent } => {
            let dot_torrent = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent =
                serde_bencode::from_bytes(&dot_torrent).context("parse torrent file")?;
            eprintln!("{:?}", t);

            println!("Tracker URL: {}", t.announce);
            println!("Length: {}", t.info.length);
            let info_hash = t.info_hash();
            println!("Info Hash: {}", hex::encode(&info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for h in t.info.pieces.0 {
                println!("{}", hex::encode(&h));
            }
        }

        Command::Peers { torrent } => {
            let dot_torrent = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent =
                serde_bencode::from_bytes(&dot_torrent).context("parse torrent file")?;
            let info_hash = t.info_hash();
            let request = TrackerRequest {
                peer_id: String::from("00112233445566778899"),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: t.info.length,
                compact: 1,
            };

            let url_params = serde_urlencoded::to_string(&request).context("url-encode")?;
            let tracker_url = format!(
                "{}?{}&info_hash={}",
                t.announce,
                url_params,
                &urlencode(&info_hash)
            );

            let response = reqwest::get(tracker_url).await.context("query tracker")?;
            let response = response.bytes().await.context("fetch tracker")?;
            let response: TrackerResponse =
                serde_bencode::from_bytes(&response).context("parse tracker")?;

            for peer in &response.peers.0 {
                println!("{}:{}", peer.ip(), peer.port());
            }
        }
    }

    Ok(())
}

fn urlencode(t: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * t.len());
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode(&[byte]));
    }

    encoded
}

mod peers {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use serde::{de::Visitor, Deserialize, Serialize};
    #[derive(Debug, Clone)]
    pub struct Peers(pub Vec<SocketAddrV4>);

    struct PeersVisitor;
    impl<'d> Visitor<'d> for PeersVisitor {
        type Value = Peers;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("6 bystes, the first 4 bystes are a peer's IP address and the last 2 are a peer's port number")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom(format!("length is {}", v.len())));
            }
            Ok(Peers(
                v.chunks_exact(6)
                    .map(|slice| {
                        SocketAddrV4::new(
                            Ipv4Addr::new(slice[0], slice[1], slice[2], slice[3]),
                            u16::from_be_bytes([slice[4], slice[5]]),
                        )
                    })
                    .collect(),
            ))
        }
    }

    impl<'d> Deserialize<'d> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'d>,
        {
            deserializer.deserialize_bytes(PeersVisitor)
        }
    }

    impl Serialize for Peers {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut single_slice = Vec::with_capacity(6 * self.0.len());
            for peer in &self.0 {
                single_slice.extend(peer.ip().octets());
                single_slice.extend(peer.port().to_be_bytes());
            }

            serializer.serialize_bytes(&single_slice)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TrackerRequest {
    peer_id: String,
    port: u16,
    uploaded: usize,
    downloaded: usize,
    left: usize,
    compact: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TrackerResponse {
    interval: usize,
    peers: Peers,
}
