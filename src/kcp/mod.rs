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

macro_rules! async_to_poll{
    ($expre: expr, $cx: expr) => {
        {
            let mut f = Box::pin($expre);
            Pin::new(&mut f).poll($cx)
        }
    }
}

#[allow(unused)]
struct KcpPair<'a>{
    k:Rc<RefCell<Kcb<KcpOutput>>>,
    set_readiness: SetReadiness,
    token: Rc<RefCell<&'a mut Timeout<Nothing>>>,
}

impl <'a>KcpPair<'a> {
    fn reset_token(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        let token_s: &'static mut Timeout<Nothing> = Box::leak(Box::new(token));
        self.token = Rc::new(RefCell::new(token_s));
    }
}


#[allow(unused)]
pub struct KcpListener<'a> {
    udp: Rc<RefCell<UdpSocket>>,
    connections: HashMap<SocketAddr, KcpPair<'a>>
}

#[allow(unused)]
impl <'a>KcpListener<'a>{
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<KcpListener<'a>>{
        
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
            status(&format!("accept wait: {:?}", self.udp.borrow()), true);
            match self.udp.borrow_mut().recv_from(&mut buf).await{
                Err(e) => {
                    status(&format!("error :[{}] ",e), false);
                    return Err(e);
                },
                Ok((n, addr)) => {
                    status(&format!("got:[{}] ",n), true);
                    if let Some(mut kp) = self.connections.get_mut(&addr){
                        let dur = {
                            let mut kcb = kp.k.borrow_mut();
                            kcb.input(&buf[..n]);
                            kcb.update(clock()).await;
                            let dur = kcb.check(clock());
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
                        let token_r = Rc::new(RefCell::new(token));
                        let to_sta:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(now, Nothing)));
                        let token_static = Rc::new(RefCell::new(to_sta));
                        
                        let stream = KcpStream {
                            kcb: kcb.clone(),
                            registration: registration,
                            set_readiness: set_readiness.clone(),
                            token: token_r.clone(),
                        };
                        let  to_sta:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(now, Nothing)));
                        let token_static = Rc::new(RefCell::new(to_sta));
                        let interval = KcpInterval {
                            kcb: kcb.clone(),
                            token: token_static.clone(),
                        };
                        stream.kcb.borrow_mut().input(&buf[..n]);

                        let kcbc = kcb.clone();
                        let mut kcb1 = kcbc.borrow_mut();
                        kcb1.update(clock()).await;
                        let dur = kcb1.check(clock());
                        **token_static.borrow_mut() = timeout_at(Instant::now() + Duration::from_millis(dur as u64), Nothing);
                        
                        stream.set_readiness.set_readiness(mio::Ready::readable());
                        let kp = KcpPair{
                            k:kcb.clone(),
                            set_readiness: set_readiness.clone(),
                            token: token_static.clone()
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
        let mut u = self.udp.borrow_mut();
        println!("-> {:?} [{}]", self.udp.borrow(), buf.len());
        u.send_to(buf, &self.peer).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncWrite for KcpOutput{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        async_to_poll!(self.get_mut().write(buf), cx)
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
        let mut udp = self.udp.borrow_mut();
        async_to_poll!(udp.recv(buf), cx)
    }
}

struct KcpInterval<'a, T> 
where T:Future
{
    kcb: Rc<RefCell<Kcb<KcpOutput>>>,
    token: Rc<RefCell<&'a mut Timeout<T>>>,
}
impl <'a>KcpInterval<'a, Nothing>{
    fn get_pin_mut_token(&mut self, ctx:&mut Context)-> Poll<Result<(), Elapsed>>{
        let mut token_mut = self.token.borrow_mut();
        let t = Pin::new(&mut *token_mut);
        t.poll(ctx)
        // Pin::new(&mut *token_mut)
    }
    fn kcp_check(&mut self, ctx:&mut Context) -> Poll::<Instant>{
        let mut kcb = self.kcb.borrow_mut();
        let mut pin_future = Box::pin(kcb.update(clock()));
        let f = Pin::new(&mut pin_future);
        match f.poll(ctx){
            Poll::Ready(_) => {
                let dur = self.kcb.borrow().check(clock());
                let next = Instant::now() + Duration::from_millis(dur as u64);
                Poll::Ready(next)
            },
            Poll::Pending => Poll::Pending
        }
        
    }
    fn reset(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        let token_static: &'static mut Timeout<Nothing> = Box::leak(Box::new(token));
        self.token = Rc::new(RefCell::new(token_static));
    }

}
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
// impl CanReset for KcpInterval<Nothing>{
    
// }

impl <'a>Future for KcpInterval<'a, Nothing>{
    type Output = Result<Nothing, Elapsed>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        match self.as_mut().get_pin_mut_token(ctx){
            Poll::Ready(test_elased) => {
                match test_elased{
                    Ok(_) => {    
                        match self.as_mut().kcp_check(ctx){
                            Poll::Ready(next) => {
                                let new_token:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(next, Nothing)));
                                self.as_mut().token = Rc::new(RefCell::new(new_token));
                                Poll::Ready(Ok(Nothing))
                            },
                            Poll::Pending => Poll::Pending
                        }
                        
                    },
                    Err(e) => Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct KcpStream {
    kcb: Rc<RefCell<Kcb<KcpOutput>>>,
    registration: Registration,
    set_readiness: SetReadiness,
    token: Rc<RefCell<Timeout<Nothing>>>,
}

impl Read for KcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.kcb.borrow_mut().recv(buf);
        match result {
            Err(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "would block")),
            Ok(n) => Ok(n),
        }
    }
}

pub struct KcpStreamNew {
    inner: Option<KcpStream>,
}

struct Server {
    socket: Rc<RefCell<UdpSocket>>,
    buf: Vec<u8>,
    to_send: Option<(usize, SocketAddr)>,
    kcb: Rc<RefCell<Kcb<KcpOutput>>>,
    set_readiness: SetReadiness,
    token: Rc<RefCell<Timeout<Nothing>>>,
}
impl Server{
    fn reset_token(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        self.token = Rc::new(RefCell::new(token));
    }
    async fn one_batch(&mut self) -> Except<()>{
        if let Some((size, peer)) = self.to_send{
            self.kcb.borrow_mut().input(&mut self.buf[..size]);
            self.kcb.borrow_mut().update(clock()).await;
            let dur = self.kcb.borrow_mut().check(clock());
            self.reset_token(
                Instant::now() +
                    Duration::from_millis(dur as u64),
            );
            self.to_send = Some(self.socket.borrow_mut().recv_from(&mut self.buf).await?);
        }
        
        Ok(())
    }
}
impl Future for Server {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut handle = self.as_mut();
        loop {
            match async_to_poll!(handle.one_batch(), cx){
                Poll::Ready(_) => {
                    self.set_readiness.set_readiness(mio::Ready::readable());
                    self.to_send = None;
                }
            }
            
            // self.to_send = Some(try_nb!(self.socket.recv_from(&mut self.buf)));
        }
    }
}

impl KcpStream{
    fn reset_token(&mut self, i: Instant){
        let token = timeout_at(i, Nothing);
        self.token = Rc::new(RefCell::new(token));
    }

    async fn flush(&mut self) -> io::Result<()>{
        let mut kcb = self.kcb.borrow_mut();
        kcb.flush().await;
        Ok(())
    }
    async fn update(&self) {
        let mut kcb = self.kcb.borrow_mut();
        kcb.update(clock()).await
    }

    async fn check(&mut self) {
        // let mut kcb = self.kcb.borrow_mut();
        let dur = self.kcb.borrow_mut().check(clock());
        self.kcb.borrow_mut().flush().await;
        self.reset_token(
            Instant::now() +
                Duration::from_millis(dur as u64),
        );
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
        let (registration, set_readiness) = Registration::new2();
        let now = Instant::now();
        let token = timeout_at(now, Nothing);
        let token = Rc::new(RefCell::new(token));
        let to_sta:&'static mut Timeout<Nothing> = Box::leak(Box::new(timeout_at(now, Nothing)));
        let token_static = Rc::new(RefCell::new(to_sta));
                        
        let stream = Self{
            kcb: kcb.clone(),
            registration: registration,
            set_readiness: set_readiness.clone(),
            token: token.clone(),
        };

        let interval = KcpInterval {
            kcb: kcb.clone(),
            token: token_static.clone()
        };
        
        stream
    }
    async fn server_read(&mut self,buf: &mut [u8]) -> io::Result<usize>{
        // let handle = self.get_mut();
        
        let r_size = self.kcb.borrow_mut().read(buf).await.unwrap();
        let size = self.kcb.borrow_mut().input(&mut buf[..r_size]);
        self.kcb.borrow_mut().update(clock()).await;
        let dur = self.kcb.borrow_mut().check(clock());
        self.reset_token(Instant::now() + Duration::from_millis(dur as u64));
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
            let mut kcb = self.kcb.borrow_mut();
            kcb.send(buf)
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

