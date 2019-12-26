use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::io;
use std::net::SocketAddr;
use std::rc::Rc;
use mio;
use mio::Registration;
use mio::SetReadiness;
use std::task::{Poll, Context};
use std::pin::Pin;
use std::future::Future;
use tokio::net::UdpSocket;
use tokio::io::AsyncWrite;
use tokio::io::AsyncRead;
use tokio::time::Duration;
use tokio::time::timeout_at;
use tokio::time::Instant;
use tokio::time::Timeout;
use tokio::net::ToSocketAddrs;
// use futures::future::poll_fn;
use crate::utils::status;

// use bytes::ByteOrder;
// use bytes::LittleEndian;
use byteorder::{ByteOrder, LittleEndian};
mod kcb;
use kcb::Kcb;
use crate::ctime;

use crate::utils::Except;
// use crate::utils::err_with;

// use std::sync::Mutex;

// struct GlobalInterval{
//     kcp_interval: Option<KcpInterval<'static,Nothing>>
// }

// lazy_static!{
//     static ref GLOBAL_INTERVAL:Mutex<GlobalInterval> = {
//         Mutex::new(GlobalInterval{
//             kcp_interval: None
//         })
//     };
// }
#[allow(unused)]
macro_rules! async_to_poll{
    ($expre: expr, $cx: expr) => {
        
        {
            let mut f = Box::pin($expre);
            Pin::new(&mut f).poll($cx)
        }
    }
}

#[allow(unused)]
struct KcpPair{
    k:Rc<RefCell<Kcb<KcpOutput>>>,
    set_readiness: SetReadiness,
    // token: Rc<RefCell<&'a mut Timeout<Nothing>>>,
    token: Rc<RefCell<Timeout<Nothing>>>,
}

#[allow(unused)]
impl KcpPair {
    fn reset_token(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        // let token_s: &'static mut Timeout<Nothing> = Box::leak(Box::new(token));
        self.token = Rc::new(RefCell::new(token));
    }
}


#[allow(unused)]
pub struct KcpListener {
    udp: Rc<RefCell<UdpSocket>>,
    connections: HashMap<SocketAddr, KcpPair>
}

#[allow(unused)]
impl KcpListener{
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<KcpListener>{
        
        // let addrs = addr.to_socket_addrs().await?;
        match UdpSocket::bind(addr).await{
            Ok(udp) => {
                Ok(KcpListener{
                    udp:Rc::new(RefCell::new(udp)),
                    connections: HashMap::new(),
                })
            },
            Err(e) => Result::Err(e)
        }   
    }

    pub fn drop_stream(&mut self, addr: SocketAddr){
        self.connections.remove(&addr);
    }

    pub async fn accept(&mut self) -> io::Result<(KcpStream, SocketAddr)>{
        let mut buf = vec![0;1024];
        loop{
            let one_recv_result = {
                let mut udp = self.udp.borrow_mut();
                // status(&format!("accept wait: {:?}", &udp), true);
                udp.recv_from(&mut buf).await
            };

            match one_recv_result{
                Err(e) => {
                    // status(&format!("error :[{}] ",e), false);
                    return Err(e);
                },
                Ok((n, addr)) => {
                    // status(&format!("got:[{}] ",n), true);
                    if let Some(mut kp) = self.connections.get_mut(&addr){
                        let mut stream = KcpStream {
                            kcb: kp.k.clone(),
                            socket: self.udp.clone(),
                            buf: vec![],
                            to_send:None,
                            server: true,
                            set_readiness: kp.set_readiness.clone(),
                            token: kp.token.clone(),
                        };
                        // stream.kcp_check().await;
                        stream.udp_recv_and_iuc_update(Some(
                            (buf[..n].to_vec(),n, addr.clone())) 
                        ).await;
                        if stream.buf.len() < 25{
                            continue
                        }
                        // status(&format!("now buf: {}",stream.buf.len()), true);
                        return Ok((stream, addr));
                    }else{
                        let conv = LittleEndian::read_u32(&buf[..4]);
                        let mut kcb = Kcb::new(
                            conv,
                            KcpOutput {
                                udp: self.udp.clone(),
                                peer: addr.clone(),
                            },
                        );
                        kcb.wndsize(128, 128);
                        kcb.nodelay(0, 10, 0, true);
                        let kcb = Rc::new(RefCell::new(kcb));
                        let (registration, set_readiness) = Registration::new2();
                        let now = Instant::now();
                        let token = timeout_at(now, Nothing);
                        let token = Rc::new(RefCell::new(token));
                        // let to_sta:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(now, Nothing)));
                        // let token_static = Rc::new(RefCell::new(to_sta));
                        
                        let mut stream = KcpStream {
                            kcb: kcb.clone(),
                            socket: self.udp.clone(),
                            buf: vec![],
                            to_send:None,
                            server: true,
                            set_readiness: set_readiness.clone(),
                            token: token.clone(),
                        };

                        stream.kcp_check().await;
                        stream.udp_recv_and_iuc_update(Some(
                            (buf[..n].to_vec(),n, addr.clone())) 
                        ).await;
                        // {
                        //     stream.kcb.borrow_mut().input(&buf[..n]);
                        // };
                        
                        // let kcbc = kcb.clone();
                        // // let mut kcb1 = ;
                        // let dur = {
                        //     let mut kcb1 = kcbc.borrow_mut();
                        //     kcb1.update(clock()).await;
                        //     kcb1.check(clock())
                        // };
                        // {
                        //     *token.borrow_mut() = timeout_at(Instant::now() + Duration::from_millis(dur as u64), Nothing);
                        // };
                        // stream.set_readiness.set_readiness(mio::Ready::readable());
                        
                        let kp = KcpPair{
                            k:kcb.clone(),
                            set_readiness: set_readiness.clone(),
                            token: token.clone()
                        };
                        self.connections.insert(addr, kp);
                        return Ok((stream, addr));
                    }
                }
            }
        }
    }

}


#[allow(unused)]
pub struct KcpOutput {
    udp: Rc<RefCell<UdpSocket>>,
    peer: SocketAddr,
}

#[allow(unused)]
impl KcpOutput {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let u = {
            let mut udp = self.udp.borrow_mut();
            udp.send_to(buf, &self.peer).await
        };
        u
    }

    async fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncWrite for KcpOutput{
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let fu = async {
            let mut handle = self.as_mut();
            handle.write(buf).await
        };
        async_to_poll!(fu, cx)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // tcp flush is a no-op
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(().into()))
    }
}

impl AsyncRead for KcpOutput{
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>>{
        let fu = async {
            let mut udp = self.udp.borrow_mut();
            udp.recv(buf).await
        };
        async_to_poll!(fu, cx)
    }
}


#[derive(Clone)]
struct Nothing;
impl Future for Nothing {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

trait CanReset{
    fn reset(&mut self, i: Instant);
}


#[derive(Clone)]
pub struct KcpStream {
    kcb: Rc<RefCell<Kcb<KcpOutput>>>,
    pub buf: Vec<u8>,
    pub server: bool,
    to_send: Option<(usize, SocketAddr)>,
    pub socket: Rc<RefCell<UdpSocket>>,
    // registration: Registration,
    set_readiness: SetReadiness,
    token: Rc<RefCell<Timeout<Nothing>>>,
}

impl Read for KcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = {
            let mut k = self.kcb.borrow_mut();
            k.recv(buf)
        };
        match result {
            Err(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "normal read would block")),
            Ok(n) => Ok(n),
        }
    }
}

// pub struct KcpStreamNew {
//     inner: Option<KcpStream>,
// }

// struct Server {
//     socket: Rc<RefCell<UdpSocket>>,
//     buf: Vec<u8>,
//     to_send: Option<(usize, SocketAddr)>,
//     kcb: Rc<RefCell<Kcb<KcpOutput>>>,
//     set_readiness: SetReadiness,
//     token: Rc<RefCell<Timeout<Nothing>>>,
// }
// impl Server{
//     fn reset_token(&mut self, i: Instant){
//         let token = timeout_at(i, Nothing);
//         self.token = Rc::new(RefCell::new(token));
//     }
//     async fn one_batch(&mut self) -> Except<()>{
//         if let Some((size, _)) = self.to_send{
//             self.kcb.borrow_mut().input(&mut self.buf[..size]);
//             self.kcb.borrow_mut().update(clock()).await;
//             let dur = self.kcb.borrow_mut().check(clock());
//             self.reset_token(
//                 Instant::now() +
//                     Duration::from_millis(dur as u64),
//             );
//             self.set_readiness.set_readiness(mio::Ready::readable());
//             self.to_send = None;
            
//         }
//         self.to_send = Some(self.socket.borrow_mut().recv_from(&mut self.buf).await?);
//         Ok(())
//     }
// }
// impl Future for Server {
//     type Output = ();

//     fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
//         let mut handle = self.get_mut();
//         loop {
//             async_to_poll!(handle.one_batch(), cx)
//             // self.to_send = Some(try_nb!(self.socket.recv_from(&mut self.buf)));
//         };
//         Poll::Ready(())
//     }
// }


impl KcpStream{
    fn reset_token(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        self.token = Rc::new(RefCell::new(token));
    }

    // input update check
    pub async fn udp_recv_and_iuc_update(&mut self, pre_to_send: Option<((Vec<u8>, usize, SocketAddr))>) -> Except<Option<SocketAddr>>{
        if let Some((buf_v, si, a)) = pre_to_send{
            self.buf = buf_v;
            self.to_send = Some((si, a));
        }

        
        if let Some((size, a)) = self.to_send{
            let dur = {
                let mut kcb = self.kcb.borrow_mut();
                let _ = kcb.input(&mut self.buf[..size]);
                kcb.update(clock()).await;
                kcb.check(clock())
            };
            
            self.reset_token(
                Instant::now() +
                    Duration::from_millis(dur as u64),
            );
            self.set_readiness.set_readiness(mio::Ready::readable())?;
            self.to_send = None;
            return Ok(Some(a));
            
        }
        Ok(None)
    }

    async fn flush(&mut self) -> io::Result<()>{
        let mut kcb = self.kcb.borrow_mut();
        kcb.flush().await;
        Ok(())
    }
    
    pub async fn kcp_check(&mut self) {
        
        let dur = {
            let mut kcb = self.kcb.borrow_mut();
            kcb.update(clock()).await;
            kcb.check(clock())
        };
        // status(&format!("next millis: {}", dur), true);
        let next = Instant::now() + Duration::from_millis(dur as u64);
        let new_token = timeout_at(next, Nothing);
        self.token = Rc::new(RefCell::new(new_token));
    }

    


    pub async fn connect(addr: &SocketAddr) -> Self{
        let r: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let udp = UdpSocket::bind(&r).await.unwrap();
        let udp = Rc::new(RefCell::new(udp));
        let conv = rand::random::<u32>();
        let mut kcb = Kcb::new(
            conv,
            KcpOutput {
                udp: udp.clone(),
                peer: addr.clone(),
            },
        );
        kcb.wndsize(128, 128);
        kcb.nodelay(0, 10, 0, true);
        let kcb = Rc::new(RefCell::new(kcb));
        let (_, set_readiness) = Registration::new2();
        let now = Instant::now();
        let token = timeout_at(now, Nothing);
        let token = Rc::new(RefCell::new(token));
        // let to_sta:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(now, Nothing)));
        // let token_static = Rc::new(RefCell::new(to_sta));
                        
        let mut stream = Self{
            kcb: kcb.clone(),
            socket: udp.clone(),
            buf: vec![],
            to_send:None,
            server: false,
            set_readiness: set_readiness.clone(),
            token: token.clone(),
        };

        (&mut stream).kcp_check().await;
        stream
    }
    async fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize>{
        // let handle = self.get_mut();
        
        if self.to_send == None || self.buf.len() == 0{
            // status("read from socket", true);
            match self.udp_recv_and_iuc_update(None).await{
                Ok(_) => {
                    // status(&format!("peer: {:?}", peer), true);
                },
                Err(e) => {
                    status(&format!("error in udp recv:{}", e), false);
                }
            }
            
            
        }
   
        let result = {
            let mut kcb = self.kcb.borrow_mut();
            kcb.recv(&mut self.buf)
        };
        
        match result {
            Err(ref e) if e.kind() == io::ErrorKind::Other => {
                // status(&format!("{}", e), false);
                Err(io::Error::new(io::ErrorKind::Other, "try read would block wait"))
            },
            Err(e) => Err(e),
            Ok(n) => {
                // buf.copy_from_slice(&self.buf[..n]);
                if n <= buf.len(){
                    for (dst, src) in buf.iter_mut().zip(&self.buf[..n]) {
                        *dst = *src;
                    }
                    self.buf = vec![];
                }else{
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "must 1024 buf!"));
                }
                self.kcp_check().await;
                Ok(n)
            },
        }
        
    }

    // async fn write_all(&mut self, buf: &mut [u8]) -> io::Result<usize>{
    //     self.write(buf).await
    // }
}



impl AsyncRead for KcpStream {
    unsafe fn prepare_uninitialized_buffer(&self, _: &mut [std::mem::MaybeUninit<u8>]) -> bool {
        false
    }

    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // let mut reader = self.kcb.borrow_mut();
        
        let is_server = self.server;
        if !is_server{
            let mut wait_or_readed = false;
            let handle = self.get_mut();
            loop{
                let f = async {
                    
                    let mut udp_socket = handle.socket.borrow_mut();
                    let mut buf = [0;1024];
                    let (u,a) = udp_socket.recv_from(&mut buf).await.unwrap();
                    handle.to_send = Some((u,a));
                    handle.buf = buf.to_vec();
                    if u > 24{
                        // status(&format!("{:?}", &buf[..]), true);
                        // handle.udp_recv_and_iuc_update(None).await;
                        wait_or_readed = true;
                    }
                };
                // let mut handle = self.get_mut();
                let res = match async_to_poll!(f, cx){
                    Poll::Ready(_) => {
                        match async_to_poll!(handle.udp_recv_and_iuc_update(None), cx){
                            Poll::Ready(_) => {
                                match async_to_poll!(handle.try_read(buf), cx){
                                    Poll::Ready(ok_or_err) => {
                                        match ok_or_err{
                                            Ok(u) => Poll::Ready(Ok(u)),
                                            Err(ref e) if e.kind() == io::ErrorKind::Other => {
                                                // status(&format!("try resume {} ", e), true);
                                                // match async_to_poll!(handle.kcp_check(), cx){
                                                //     Poll::Ready(_) => Poll::Pending,
                                                //     Poll::Pending => Poll::Pending
                                                // }
                                                Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "EOF")))
                                            },
                                            Err(e) => {
                                                Poll::Ready(Err(e))
                                            }
                                        }
                                    },
                                    Poll::Pending => Poll::Pending,
                                }
                            },
                            Poll::Pending => Poll::Pending,
                        }
                    },
                    Poll::Pending => Poll::Pending
                };
                if wait_or_readed{
                    return res;
                }
                
            }
            
            
        }else{
            async_to_poll!(self.get_mut().try_read(buf), cx)
        }
        
        
        
    }
}

impl AsyncWrite for KcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = {
            self.kcb.borrow_mut().send(buf)
        };
        // let b = self.get_mut();
        let future = async {
            let dur = {
                let mut kcb = self.kcb.borrow_mut();
                kcb.update(clock()).await;
                let dur = kcb.check(clock());
                kcb.flush().await;
                dur
            };
            self.get_mut().reset_token(Instant::now() +
                Duration::from_millis(dur as u64));
        };
        match async_to_poll!(future, cx){
            Poll::Ready(_) => {
                Poll::Ready(result)
            },
            Poll::Pending => Poll::Pending
        }
    }

    

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // tcp flush is a no-op
        // Poll::Ready(Ok(()))
        async_to_poll!(self.get_mut().flush(), cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(().into()))
    }
}


#[inline]
fn clock() -> u32 {
    let timespec = ctime::get_time();
    let mills = timespec.sec * 1000 + timespec.nsec as i64 / 1000 / 1000;
    mills as u32
}

