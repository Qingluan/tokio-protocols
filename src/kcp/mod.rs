use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::rc::Rc;
// use std::cell::RefMut;
// use std::time::{Duration};

// use std::future::AsyncWrite;
// use mio::event::Evented;
use mio::{self,Ready, Registration, PollOpt, Token, SetReadiness};

use std::task::{Poll, Context};
use std::pin::Pin;
use std::future::Future;

use tokio::net::UdpSocket;

use tokio::io::AsyncWrite;
// use tokio::io::AsyncWriteExt;
use tokio::io::AsyncRead;
// use tokio::io::AsyncReadExt;
use tokio::time::Duration;
// use tokio::io::PollEvented;
use tokio::time::timeout_at;
use tokio::time::Instant;
use tokio::time::Elapsed;
use tokio::time::Timeout;
use tokio::net::ToSocketAddrs;
use futures::future::poll_fn;

// use futures::async_await::PollOnce;
use crate::utils::status;

use bytes::{Buf, BufMut, ByteOrder, LittleEndian};
mod kcb;
use kcb::Kcb;
use crate::ctime;

use crate::utils::Except;
use crate::utils::err_with;

use std::sync::Mutex;

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

    pub async fn accept(&mut self) -> io::Result<(KcpStream, SocketAddr)>{
        let mut buf = vec![0;1024];
        loop{
            let one_recv_result = {
                let mut udp = self.udp.borrow_mut();
                status(&format!("accept wait: {:?}", &udp), true);
                udp.recv_from(&mut buf).await
            };

            match one_recv_result{
                Err(e) => {
                    status(&format!("error :[{}] ",e), false);
                    return Err(e);
                },
                Ok((n, addr)) => {
                    status(&format!("got:[{}] ",n), true);
                    if let Some(mut kp) = self.connections.get_mut(&addr){
                        {
                            let mut k = kp.k.borrow_mut();
                            k.input(&buf[..n]);
                            k.update(clock()).await;
                            
                        };
                        let dur = {
                            let dur = kp.k.borrow().check(clock());
                            dur
                        };
                        kp.reset_token(
                            Instant::now() +
                                Duration::from_millis(dur as u64),
                        );
                        kp.set_readiness.set_readiness(mio::Ready::readable());
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
                        
                        let stream = KcpStream {
                            kcb: kcb.clone(),
                            // registration: registration,
                            set_readiness: set_readiness.clone(),
                            token: token.clone(),
                        };
                        {
                            stream.kcb.borrow_mut().input(&buf[..n]);
                        };
                        


                        let kcbc = kcb.clone();
                        // let mut kcb1 = ;
                        let dur = {
                            let mut kcb1 = kcbc.borrow_mut();
                            kcb1.update(clock()).await;
                            kcb1.check(clock())
                        };
                        {
                            *token.borrow_mut() = timeout_at(Instant::now() + Duration::from_millis(dur as u64), Nothing);
                        };
                        
                        
                        stream.set_readiness.set_readiness(mio::Ready::readable());
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
            Err(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "would block")),
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

    
    pub async fn kcp_check(&mut self) {
        
        let dur = {
            let mut kcb = self.kcb.borrow_mut();
            kcb.update(clock()).await;
            kcb.check(clock())
        };
        status(&format!("next millis: {}", dur), true);
        let next = Instant::now() + Duration::from_millis(dur as u64);
        let new_token = timeout_at(next, Nothing);
        self.token = Rc::new(RefCell::new(new_token));
    }

    async fn flush(&mut self) -> io::Result<()>{
        self.kcb.borrow_mut().flush().await;
        Ok(())
    }
    async fn update(&self) {
        let mut kcb = self.kcb.borrow_mut();
        kcb.update(clock()).await
    }

    async fn check(&mut self) {
        let dur = {
            self.kcb.borrow().check(clock())
        };
        let _ = self.flush().await;
        
        let token = timeout_at( Instant::now() +
        Duration::from_millis(dur as u64), Nothing);
        self.token = Rc::new(RefCell::new(token));
    }


    pub async fn connect(addr: &SocketAddr) -> Self{
        let r: SocketAddr = "127.0.0.1:0".parse().unwrap();
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
            // registration: registration,
            set_readiness: set_readiness.clone(),
            token: token.clone(),
        };
        // let interval = KcpInterval {
        //         kcb: kcb.clone(),
        //         token: Rc::new(RefCell::new(Box::leak(Box::new(timeout_at(now, Nothing)))))
        //     };
        
        // tokio::spawn(async move{
        //     loop_kcp_interval(interval.clone())
        // });
        // tokio::spawn(async move{
        //     let now = Instant::now();
        //     let interval = KcpInterval {
        //         kcb: kcb.clone(),
        //         token: Rc::new(RefCell::new(Box::leak(Box::new(timeout_at(now, Nothing)))))
        //     };
            
        //     loop{
        //         // interval.await;
        //         interval.await;
        //     }
        // });
        // tokio::spawn(async move  {
        //     let mut stream_shared = stream.clone();
        // });
        (&mut stream).kcp_check().await;
        stream
    }
    async fn server_read(&mut self,buf: &mut [u8]) -> io::Result<usize>{
        // let handle = self.get_mut();
        let mut kcb = self.kcb.borrow_mut();
        let r_size = kcb.read(buf).await.unwrap();
        let size = kcb.input(&mut buf[..r_size]);
        kcb.update(clock()).await;
        let dur = kcb.check(clock());
        status(&format!("next millis: {}", dur), true);
        let next = Instant::now() + Duration::from_millis(dur as u64);
        let new_token = timeout_at(next, Nothing);
        self.token = Rc::new(RefCell::new(new_token));
        let _ = self.set_readiness.set_readiness(mio::Ready::readable());
        size
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
        let handle = self.get_mut();
        async_to_poll!(handle.server_read(buf), cx)
        
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
        let b = self.get_mut();
        match async_to_poll!(b.update(), cx){
            Poll::Ready(_) => {
                match async_to_poll!(b.check(), cx){
                    Poll::Ready(_) =>Poll::Ready(result),
                    Poll::Pending => Poll::Pending
                }
            },
            Poll::Pending => Poll::Pending
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // tcp flush is a no-op
        // Poll::Ready(Ok(()));
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

