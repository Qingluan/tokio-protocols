extern crate tokio;
extern crate rust_net_protocols;
use colored::Colorize;
use rust_net_protocols::kcp;
use rust_net_protocols::kcp::KcpStream;
use rust_net_protocols::utils;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;

use utils::Except;
use utils::err_with;
use std::env;
use std::str;



#[tokio::main]
async fn main()  -> Except<()>{
    let addr = env::args().nth(1).unwrap_or_else(|| {
        panic!("this program requires at least one argument")
    });
    println!("run in {}", addr);
    let mut listener = kcp::KcpListener::bind(addr).await?;
    let mut c = 0;let mut ec = 0;
    loop{
        let (mut stream, addr) = listener.accept().await?;
        c+=1;
        match kcp_handle(&mut stream,(ec, c)).await{
            Ok(_) => c+=1,
            Err(e) => { ec +=1; println!("failed to read from socket; err = {:?}", e);listener.drop_stream(addr);}
        };
    }
}

async fn kcp_handle(stream: &mut KcpStream, counter: (i32, i32)) -> Except<()>{
    let mut buf = [0; 1024];
    let (ec, c) = counter;
    match stream.read(&mut buf).await{
        Ok(n) if n == 0 => return Ok(()),
        Ok(n) => {
            stream.write_all(&buf[..n]).await?;
            print!("{} | {}\r",&format!("recv: {}/{} err/all ",ec,c ).red(), str::from_utf8(&buf[..n]).unwrap().trim().green());      
            Ok(())
        },
        Err(e) => {
            Err(err_with(&format!("failed to read from socket; err = {:?}", e)))
        }
    }
}
