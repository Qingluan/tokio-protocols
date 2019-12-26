extern crate tokio;
extern crate rust_net_protocols;

use rust_net_protocols::kcp;
use rust_net_protocols::utils;
use rust_net_protocols::utils::status;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;
use std::net::SocketAddr;

// use tokio::time::Duration;
use tokio::time::Instant;
use std::env;
use utils::Except;
use std::str;

#[tokio::main]
async fn main()  -> Except<()>{
    let addr = env::args().nth(1).unwrap_or_else(|| {
        panic!("this program requires at least one argument")
    });
    let sock_addr:SocketAddr = addr.parse().expect("parse sockset error"); 
    let mut stream = kcp::KcpStream::connect(&sock_addr).await;
    utils::status("connected", true);
    let mut c = 0;
    loop{
        let now = Instant::now();
        let mut buf = [0;1024];
        let _ = stream.write_all(b"hello world\n").await;
        
        let size = stream.read(&mut buf).await?;
        let new_now = Instant::now();
        status(&format!("[{}]| {:?} | read : {} ",c, new_now.duration_since(now) ,str::from_utf8(&mut buf[..size])?.trim()), true);
        c+=1;
    }
}