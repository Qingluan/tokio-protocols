extern crate tokio;
extern crate rust_net_protocols;

use rust_net_protocols::kcp;
use rust_net_protocols::utils;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;

use utils::Except;
use std::str;

#[tokio::main]
async fn main()  -> Except<()>{
    let mut listener = kcp::KcpListener::bind("127.0.0.1:8080").await?;
    
    loop{
        utils::status("wait", true);
        let mut buf = [0; 4096];
        let (mut stream, _) = listener.accept().await?;
        utils::status("got", true);
        let _ = stream.write_all(b"hello world\n").await;
        let u = stream.read(&mut buf).await?;
        utils::status(str::from_utf8(&mut buf[..u])?, true);
        
    }
}