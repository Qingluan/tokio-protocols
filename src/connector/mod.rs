use tokio::io::AsyncWrite;
use tokio::io::AsyncRead;
use tokio::net::TcpStream;
use std::marker::Unpin;
use std::io;

use crate::socks5::Address;
use crate::utils::*;

pub struct TcpConnector<A,B>
where A: AsyncRead + Unpin,
    B: AsyncWrite + Unpin,
{
    from_socket: A,
    to_socket: B,
}

impl <A,B>TcpConnector<A,B>
where A: AsyncRead + Unpin,
    B: AsyncWrite + Unpin,
{
 
    pub async fn tcp_connector(mut stream:A) -> Except<()>{
        let remote_addr = match Address::read_from(&mut stream).await {
            Ok(o) => o,
            Err(err) => {
                err_with(&format!(
                    "Failed to decode Address, may be wrong method or key, peer ",
                    
                ));
                return Err(From::from(err));
            }
        };

        let mut remote_stream = match remote_addr {
            Address::SocketAddress(ref saddr) => {
                
    
                match TcpStream::connect(saddr).await {
                    Ok(s) => {
                        err_with(&format!("Connected to remote {}", saddr));
                        s
                    }
                    Err(err) => {
                        err_with(&format!("Failed to connect remote {}, {}", saddr, err));
                        return Err(err);
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
                        return Err(err);
                    }
                };
    
                // Still need to check forbidden IPs
                // let forbidden_ip = &context.config().forbidden_ip;
                // if !forbidden_ip.is_empty() {
                //     let peer_addr = s.peer_addr()?;
    
                //     if forbidden_ip.contains(&peer_addr.ip()) {
                //         error!("{} is forbidden, failed to connect {}", peer_addr.ip(), peer_addr);
                //         let err = io::Error::new(
                //             io::ErrorKind::Other,
                //             format!("{} is forbidden, failed to connect {}", peer_addr.ip(), peer_addr),
                //         );
                //         return Err(err);
                //     }
                // }
    
                s
            }
        };
    
        debug!("Relay {} <-> {} established", peer_addr, remote_addr);
    
        let (mut cr, mut cw) = stream.split();
        let (mut sr, mut sw) = remote_stream.split();
    
        use tokio::io::copy;
    
        // CLIENT -> SERVER
        let rhalf = copy(&mut cr, &mut sw);
    
        // CLIENT <- SERVER
        let whalf = copy(&mut sr, &mut cw);
    
        match future::select(rhalf, whalf).await {
            Either::Left((Ok(_), _)) => trace!("Relay {} -> {} closed", peer_addr, remote_addr),
            Either::Left((Err(err), _)) => trace!("Relay {} -> {} closed with error {:?}", peer_addr, remote_addr, err),
            Either::Right((Ok(_), _)) => trace!("Relay {} <- {} closed", peer_addr, remote_addr),
            Either::Right((Err(err), _)) => trace!("Relay {} <- {} closed with error {:?}", peer_addr, remote_addr, err),
        }

        Ok(())
    
    }
}