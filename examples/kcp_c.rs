extern crate tokio;
extern crate rust_net_protocols;

use rust_net_protocols::kcp;
use rust_net_protocols::utils;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;
use std::net::SocketAddr;

use utils::Except;
use std::str;

#[tokio::main]
async fn main()  -> Except<()>{
    let sock_addr:SocketAddr = "127.0.0.1:8080".parse().expect("parse sockset error"); 
    let mut stream = kcp::KcpStream::connect(&sock_addr).await;
    stream.kcp_check().await;
    utils::status("connected", true);
    loop{
        let mut buf = [0;4096];
        let size = stream.read(&mut buf).await?;
        utils::status(str::from_utf8(&mut buf[..size])?, true);
        let _ = stream.write_all(b"hello world\n").await;

    }
}