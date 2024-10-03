use serde_json;
use std::{env, usize};

// Available if you need it!
// use serde_bencode

#[allow(dead_code)]
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
        _ => {}
    }

    panic!("Unhandled encoded value: {}", encoded_value)
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        // You can use print statements as follows for debugging, they'll be visible when running tests.
        eprintln!("Logs from your program will appear here!");

        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{:?}", decoded_value.0.to_string());
    } else {
        eprintln!("unknown command: {}", args[1])
    }
}
