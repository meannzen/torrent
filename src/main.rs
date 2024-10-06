use anyhow::Context;
use clap::{Parser, Subcommand};
use core::panic;
use serde::{de::Visitor, Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Torrent {
    announce: String,
    info: Info,
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
}

fn main() -> anyhow::Result<()> {
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

            let info_encode = serde_bencode::to_bytes(&t.info).context("re-encode info section")?;
            let mut hasher = Sha1::new();
            hasher.update(&info_encode);
            let info_hash = hasher.finalize();
            println!("Info Hash: {}", hex::encode(&info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for h in t.info.pieces.0 {
                println!("{}", hex::encode(&h));
            }
        }
    }

    Ok(())
}
