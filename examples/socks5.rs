extern crate tokio;
extern crate time as ctime;
extern crate mio;
extern crate futures;
extern crate rust_net_protocols;
use tokio::net::TcpListener;
use tokio::prelude::*;

use rust_net_protocols::socks5;
use rust_net_protocols::utils;
// mod utils;
// mod socks5;
// mod kcp;

use utils::Except;


#[tokio::main]
async fn main()  -> Except<()>{
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;
    loop{
        let (socket, _) = listener.accept().await?;
        let (mut socket, sock5) =  socks5::Socks5::acceptor(socket).await?;
        println!("connect addr:{} | port:{} | use_host:{}", &sock5.addr, sock5.port, sock5.use_host);
        tokio::spawn(async move{
            let mut buf = [0;4096];
            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        println!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                // Write the data back
                if let Err(e) = socket.write_all(&buf[0..n]).await {
                    println!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}
