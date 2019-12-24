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
        utils::status("wait", true);
        let mut buf = [0; 4096];
        let (mut stream, _) = listener.accept().await?;
        // utils::status("got", true);
        'st: loop{
            
            let u = match stream.read(&mut buf).await{
                Ok(n) if n == 0 => return Ok(()),
                Ok(n) => n,
                Err(e) => {
                    println!("failed to read from socket; err = {:?}", e);
                    break 'st;
                }
            };
            status(&format!("buf: {:?}", &buf[..u]), true);
            let _ = stream.write_all(&buf[..u]).await;
            
            // match str::from_utf8(&mut buf[..u]){
            //     Ok(msg) => status(msg, true),
            //     Err(e) => {
            //         status(&format!("{}",e), false);
            //         break 'st;
            //     }
                
            // };
        }
        
        
    }
}