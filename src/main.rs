use anyhow::Context;
use bittorrent_starter_rust::{decode_bencoded_value, urlencode, Torrent, TrackerRequest};
use clap::{Parser, Subcommand};
use std::{net::SocketAddrV4, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    Handshake { torrent: PathBuf, peer: String },
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
            println!("Info Hash: {}", hex::encode(info_hash));
            println!("Piece Length: {}", t.info.plength);
            println!("Piece Hashes:");
            for h in t.info.pieces.0 {
                println!("{}", hex::encode(h));
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
            let response: bittorrent_starter_rust::TrackerResponse =
                serde_bencode::from_bytes(&response).context("parse tracker")?;

            for peer in &response.peers.0 {
                println!("{}:{}", peer.ip(), peer.port());
            }
        }
        Command::Handshake { torrent, peer } => {
            let dot_torrent = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent =
                serde_bencode::from_bytes(&dot_torrent).context("parse torrent file")?;
            let info_hash = t.info_hash();
            let socket_address = peer.parse::<SocketAddrV4>().context("parse address")?;
            let mut tcp = tokio::net::TcpStream::connect(socket_address)
                .await
                .context("connection")?;

            let mut handshake = Handshake {
                info_hash,
                bittorrent: *b"BitTorrent protocol",
                length: 19,
                peer_id: *b"00112233445566778899",
                reserved: [0; 8],
            };

            {
                let handshake_bytes =
                    &mut handshake as *mut Handshake as *mut [u8; std::mem::size_of::<Handshake>()];
                let handshake_bytes: &mut [u8; std::mem::size_of::<Handshake>()] =
                    unsafe { &mut *handshake_bytes };
                tcp.write_all(handshake_bytes)
                    .await
                    .context("write handshake")?;
                tcp.read_exact(handshake_bytes)
                    .await
                    .context("read handshake")?;
            }
            assert_eq!(handshake.length, 19);
            assert_eq!(&handshake.bittorrent, b"BitTorrent protocol");
            println!("Peer ID: {}", hex::encode(&handshake.peer_id));
        }
    }

    Ok(())
}

#[derive(Debug)]
#[repr(C)]
struct Handshake {
    length: u8,
    info_hash: [u8; 20],
    reserved: [u8; 8],
    peer_id: [u8; 20],
    bittorrent: [u8; 19],
}
