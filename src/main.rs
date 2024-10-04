use anyhow::Context;
use clap::{Parser, Subcommand};
use core::panic;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Torrent {
    announce: String,
    info: Info,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Info {
    name: String,
    #[serde(rename = "piece length")]
    plength: usize,
    length: usize,
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
        }
    }

    Ok(())
}
