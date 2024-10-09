use serde::{de::Visitor, Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddrV4};
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
