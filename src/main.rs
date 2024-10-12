use anyhow::Context;
use bittorrent_starter_rust::{decode_bencoded_value, urlencode, Keys, Torrent, TrackerRequest};
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
    Decode {
        value: String,
    },
    Info {
        torrent: PathBuf,
    },
    Peers {
        torrent: PathBuf,
    },
    Handshake {
        torrent: PathBuf,
        peer: String,
    },
    DownloadPiece {
        #[arg(short)]
        path: PathBuf,
        torrent: PathBuf,
        piece: usize,
    },
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
            match t.info.keys {
                Keys::SingleFile { length } => {
                    println!("Length: {}", length);
                    let info_hash = t.info_hash();
                    println!("Info Hash: {}", hex::encode(info_hash));
                    println!("Piece Length: {}", t.info.plength);
                    println!("Piece Hashes:");
                    for h in t.info.pieces.0 {
                        println!("{}", hex::encode(h));
                    }
                }
                _ => {
                    todo!()
                }
            }
        }

        Command::Peers { torrent } => {
            let dot_torrent = std::fs::read(torrent).context("read torrent file")?;
            let t: Torrent =
                serde_bencode::from_bytes(&dot_torrent).context("parse torrent file")?;
            let length = match t.info.keys {
                Keys::SingleFile { length } => length,
                _ => {
                    todo!();
                }
            };
            let info_hash = t.info_hash();
            let request = TrackerRequest {
                peer_id: String::from("00112233445566778899"),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: length,
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
                length: 19,
                bittorrent: *b"BitTorrent protocol",
                reserved: [0; 8],
                info_hash,
                peer_id: *b"00112233445566778899",
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

            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        }
        Command::DownloadPiece {
            path: _,
            torrent: _,
            piece: _,
        } => {
            // I don't know how to implment this just learn first
            todo!()
        }
    }

    Ok(())
}

#[repr(C)]
struct Handshake {
    length: u8,
    bittorrent: [u8; 19],
    reserved: [u8; 8],
    info_hash: [u8; 20],
    peer_id: [u8; 20],
}
