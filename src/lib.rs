mod peer;
mod torrent;
mod tracker;

pub use peer::Peers;
pub use torrent::*;
pub use tracker::*;

pub fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
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

pub fn urlencode(t: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * t.len());
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode([byte]));
    }

    encoded
}
