extern crate tokio;
extern crate rust_net_protocols;

use rust_net_protocols::kcp;
use rust_net_protocols::utils;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;

use utils::Except;
use utils::status;
use std::str;

#[tokio::main]
async fn main()  -> Except<()>{
    let mut listener = kcp::KcpListener::bind("0.0.0.0:8080").await?;
    
    loop{
        // utils::status("wait", true);
        let mut buf = [0; 4096];
        let (mut stream, addr) = listener.accept().await?;
        // utils::status("got", true);
        // 'st: loop{
        
        match stream.read(&mut buf).await{
            Ok(n) if n == 0 => return Ok(()),
            Ok(n) => {
                let _ = stream.write_all(&buf[..n]).await;
                status(str::from_utf8(&buf[..n]).unwrap(), true);
                stream.write_all(b"hello").await;
            },
            Err(e) => {
                println!("failed to read from socket; err = {:?}", e);
                listener.drop_stream(addr);
            }
        };
        // status(&format!("read: {:?}", &buf[..u]), true);
        

        // }
        
        
        
    }
}