// use tokio::io::AsyncWrite;
use tokio::io::AsyncRead;
// use tokio::io::ReadHalf;
// use tokio::io::WriteHalf;
use tokio::net::TcpStream;
use std::marker::Unpin;
// use std::io;
use log::{debug, error, trace};
use futures::{
    future::{self, Either},
    // stream::{FuturesUnordered, StreamExt},
};
use crate::socks5::Address;
use crate::utils::*;

pub struct TcpConnector;
// pub trait CanSplit{
//     fn split<T>(self) -> (ReadHalf<T>, WriteHalf<T>) where T: AsyncRead+ AsyncWrite;
// }


impl TcpConnector{
 
    pub async fn new<R>(mut stream:R) -> Except<(TcpStream,R)>
    where R: AsyncRead + Unpin 
    {
        let remote_addr = match Address::read_from(&mut stream).await {
            Ok(o) => o,
            Err(err) => {
                err_with(&format!(
                    "Failed to decode Address, may be wrong method or key, peer ",
                    
                ));
                return Err(From::from(err));
            }
        };
        let remote_stream = match remote_addr {
            Address::SocketAddress(ref saddr) => {
                match TcpStream::connect(saddr).await {
                    Ok(s) => {
                        err_with(&format!("Connected to remote {}", saddr));
                        s
                    }
                    Err(err) => {
                        err_with(&format!("Failed to connect remote {}, {}", saddr, err));
                        return Err(Box::new(err));
                    }
                }
            }
            #[cfg(feature = "trust-dns")]
            Address::DomainNameAddress(ref dname, port) => {
                use crate::relay::dns_resolver::resolve;
    
                let addrs = match resolve(&*context, dname.as_str(), port, true).await {
                    Ok(r) => r,
                    Err(err) => {
                        error!("Failed to resolve {}, {}", dname, err);
                        return Err(err);
                    }
                };
    
                let mut last_err: Option<io::Error> = None;
                let mut stream_opt = None;
                for addr in &addrs {
                    match TcpStream::connect(addr).await {
                        Ok(s) => stream_opt = Some(s),
                        Err(err) => {
                            error!(
                                "Failed to connect remote {}:{} (resolved: {}), {}, try others",
                                dname, port, addr, err
                            );
                            last_err = Some(err);
                        }
                    }
                }
                match stream_opt {
                    Some(s) => {
                        debug!("Connected to remote {}:{}", dname, port);
                        s
                    }
                    None => {
                        let err = last_err.unwrap();
                        error!("Failed to connect remote {}:{}, {}", dname, port, err);
                        return Err(io::Error::new(io::ErrorKind::Other, err));
                    }
                }
            }
            #[cfg(not(feature = "trust-dns"))]
            Address::DomainNameAddress(ref dname, port) => {
                let s = match TcpStream::connect((dname.as_str(), port)).await {
                    Ok(s) => {
                        debug!("Connected to remote {}:{}", dname, port);
                        s
                    }
                    Err(err) => {
                        error!("Failed to connect remote {}:{}, {}", dname, port, err);
                        return Err(Box::new(err));
                    }
                };
                s
            }
        };
    
        debug!("Relay  <-> {} established",  remote_addr);
        Ok((remote_stream,stream))
    
    }

    pub async fn connect(stream: &mut TcpStream, remote_stream:&mut TcpStream) -> Except<()>{

        let (mut cr, mut cw) = stream.split();
        let (mut sr, mut sw) = remote_stream.split();
    
        use tokio::io::copy;
    
        // CLIENT -> SERVER
        let rhalf = copy(&mut cr, &mut sw);
    
        // CLIENT <- SERVER
        let whalf = copy(&mut sr, &mut cw);
    
        match future::select(rhalf, whalf).await {
            Either::Left((Ok(_), _)) => trace!(" -> locl closed"),
            Either::Left((Err(err), _)) => trace!(" -> {} closed with error {:?}",  "remote_addr", err),
            Either::Right((Ok(_), _)) => trace!(" <- {} closed",  "remote_addr"),
            Either::Right((Err(err), _)) => trace!(" <- {} closed with error {:?}", "remote_addr", err),
        }
        Ok(())
    }
}