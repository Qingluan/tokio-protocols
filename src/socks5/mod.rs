
use tokio::prelude::*;
use tokio::net::{self,TcpStream};
use crate::utils::Except;
use crate::utils::err_with;
use crate::utils::status;
use byteorder::{NetworkEndian};
use byteorder::ByteOrder;
use std::str;
use log::error;


use std::{
    convert::From,
    error,
    fmt::{self, Debug, Formatter},
    io::{self, Cursor},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
    str::FromStr,
    u8,
    vec,
};

use bytes::{buf::BufExt,Buf, BufMut, BytesMut};
// use log::error;

#[rustfmt::skip]
#[allow(unused)]
mod consts {
    pub const SOCKS5_VERSION:                          u8 = 0x05;

    pub const SOCKS5_AUTH_METHOD_NONE:                 u8 = 0x00;
    pub const SOCKS5_AUTH_METHOD_GSSAPI:               u8 = 0x01;
    pub const SOCKS5_AUTH_METHOD_PASSWORD:             u8 = 0x02;
    pub const SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE:       u8 = 0xff;

    pub const SOCKS5_CMD_TCP_CONNECT:                  u8 = 0x01;
    pub const SOCKS5_CMD_TCP_BIND:                     u8 = 0x02;
    pub const SOCKS5_CMD_UDP_ASSOCIATE:                u8 = 0x03;

    pub const SOCKS5_ADDR_TYPE_IPV4:                   u8 = 0x01;
    pub const SOCKS5_ADDR_TYPE_DOMAIN_NAME:            u8 = 0x03;
    pub const SOCKS5_ADDR_TYPE_IPV6:                   u8 = 0x04;

    pub const SOCKS5_REPLY_SUCCEEDED:                  u8 = 0x00;
    pub const SOCKS5_REPLY_GENERAL_FAILURE:            u8 = 0x01;
    pub const SOCKS5_REPLY_CONNECTION_NOT_ALLOWED:     u8 = 0x02;
    pub const SOCKS5_REPLY_NETWORK_UNREACHABLE:        u8 = 0x03;
    pub const SOCKS5_REPLY_HOST_UNREACHABLE:           u8 = 0x04;
    pub const SOCKS5_REPLY_CONNECTION_REFUSED:         u8 = 0x05;
    pub const SOCKS5_REPLY_TTL_EXPIRED:                u8 = 0x06;
    pub const SOCKS5_REPLY_COMMAND_NOT_SUPPORTED:      u8 = 0x07;
    pub const SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;
}


#[allow(unused)]
const A_IPV4:u8 = 0x01;
#[allow(unused)]
const A_IPV6:u8 = 0x04;
#[allow(unused)]
const A_HOST:u8 =0x03;

pub struct Socks5{
    pub addr:String,
    pub port:i16,
    pub use_host:bool
}

#[allow(unused)]
impl Socks5 {
    pub async fn acceptor(mut socket: net::TcpStream) -> Except<(TcpStream, Self)>{
        let mut buf = [0;4096];
        
        // step 1 hello
        let n = socket.read(&mut buf).await?;
        if n < 3 || buf[0] != 5  || buf[1] > 1{
            println!("{:?}",&buf[..n]);
            return Result::Err(err_with("not valid socks5 hello!"));
        }else{
            status("hello",true);
            socket.write_all(b"\x05\x00").await?;
        }

        //step 2 wait connect
        let n = socket.read(&mut buf).await?;
        let method = buf[1];
        if method != 1{
            println!("{:?}",&buf[..n]);
            return Result::Err(err_with("no valid method for socks5"))
        }
        status("cmd",true);
        let data = &buf[3..n];
        let mut sock5_handle = Socks5{addr:"".to_string(), port:0, use_host:false};
        sock5_handle.parse_header(data)?;
        status("addr",true);
        socket.write_all(b"\x05\x00\x00\x01\x00\x00\x00\x00\x10\x10").await?;
        Ok((socket,sock5_handle))
    }

    fn parse_header(&mut self, data: &[u8]) -> Except<()>{
        
        // println!("{:?}", data);
        match data[0]{
            A_IPV4 =>{
                // let mut host :String = String::from("");
                let host = data[1..5].iter().fold("".to_string(), |mut acc,c|{
                    acc.push_str(".");
                    acc.push_str(&c.to_string());
                    acc
                });
                let port = NetworkEndian::read_i16(&data[5..7]);
                self.addr = host;
                self.port = port;
            },
            A_HOST =>{
                let len = data[1] as usize;
                let host = str::from_utf8(&data[2..len+2])?;
                let port = NetworkEndian::read_i16(&data[len+2..len+4]);
                self.addr = host.to_string();
                self.port = port;
                self.use_host = true;
            },
            _ => {return Result::Err(err_with("not support addr type "));}
        }
        Ok(())
    }
}


/// SOCKS5 reply code
#[derive(Clone, Debug, Copy)]
pub enum Reply {
    Succeeded,
    GeneralFailure,
    ConnectionNotAllowed,
    NetworkUnreachable,
    HostUnreachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddressTypeNotSupported,

    OtherReply(u8),
}
#[allow(unused)]
impl Reply {
    #[inline]
    #[rustfmt::skip]
    fn as_u8(self) -> u8 {
        match self {
            Reply::Succeeded               => consts::SOCKS5_REPLY_SUCCEEDED,
            Reply::GeneralFailure          => consts::SOCKS5_REPLY_GENERAL_FAILURE,
            Reply::ConnectionNotAllowed    => consts::SOCKS5_REPLY_CONNECTION_NOT_ALLOWED,
            Reply::NetworkUnreachable      => consts::SOCKS5_REPLY_NETWORK_UNREACHABLE,
            Reply::HostUnreachable         => consts::SOCKS5_REPLY_HOST_UNREACHABLE,
            Reply::ConnectionRefused       => consts::SOCKS5_REPLY_CONNECTION_REFUSED,
            Reply::TtlExpired              => consts::SOCKS5_REPLY_TTL_EXPIRED,
            Reply::CommandNotSupported     => consts::SOCKS5_REPLY_COMMAND_NOT_SUPPORTED,
            Reply::AddressTypeNotSupported => consts::SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED,
            Reply::OtherReply(c)           => c,
        }
    }

    #[inline]
    #[rustfmt::skip]
    fn from_u8(code: u8) -> Reply {
        match code {
            consts::SOCKS5_REPLY_SUCCEEDED                  => Reply::Succeeded,
            consts::SOCKS5_REPLY_GENERAL_FAILURE            => Reply::GeneralFailure,
            consts::SOCKS5_REPLY_CONNECTION_NOT_ALLOWED     => Reply::ConnectionNotAllowed,
            consts::SOCKS5_REPLY_NETWORK_UNREACHABLE        => Reply::NetworkUnreachable,
            consts::SOCKS5_REPLY_HOST_UNREACHABLE           => Reply::HostUnreachable,
            consts::SOCKS5_REPLY_CONNECTION_REFUSED         => Reply::ConnectionRefused,
            consts::SOCKS5_REPLY_TTL_EXPIRED                => Reply::TtlExpired,
            consts::SOCKS5_REPLY_COMMAND_NOT_SUPPORTED      => Reply::CommandNotSupported,
            consts::SOCKS5_REPLY_ADDRESS_TYPE_NOT_SUPPORTED => Reply::AddressTypeNotSupported,
            _                                               => Reply::OtherReply(code),
        }
    }
}

impl fmt::Display for Reply {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Reply::Succeeded               => write!(f, "Succeeded"),
            Reply::AddressTypeNotSupported => write!(f, "Address type not supported"),
            Reply::CommandNotSupported     => write!(f, "Command not supported"),
            Reply::ConnectionNotAllowed    => write!(f, "Connection not allowed"),
            Reply::ConnectionRefused       => write!(f, "Connection refused"),
            Reply::GeneralFailure          => write!(f, "General failure"),
            Reply::HostUnreachable         => write!(f, "Host unreachable"),
            Reply::NetworkUnreachable      => write!(f, "Network unreachable"),
            Reply::OtherReply(u)           => write!(f, "Other reply ({})", u),
            Reply::TtlExpired              => write!(f, "TTL expired"),
        }
    }
}

/// SOCKS5 protocol error
#[derive(Clone)]
pub struct Error {
    /// Reply code
    pub reply: Reply,
    /// Error message
    pub message: String,
}

impl Error {
    pub fn new(reply: Reply, message: &str) -> Error {
        Error {
            reply,
            message: message.to_string(),
        }
    }
}

impl Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.message[..]
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::new(Reply::GeneralFailure, <io::Error as error::Error>::description(&err))
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.message)
    }
}



#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Address {
    /// Socket address (IP Address)
    SocketAddress(SocketAddr),
    /// Domain name address
    DomainNameAddress(String, u16),
}

impl Address {
    pub async fn read_from<R>(stream: &mut R) -> Result<Address, Error>
    where
        R: AsyncRead + Unpin,
    {
        let mut addr_type_buf = [0u8; 1];
        let _ = stream.read_exact(&mut addr_type_buf).await?;

        let addr_type = addr_type_buf[0];
        match addr_type {
            consts::SOCKS5_ADDR_TYPE_IPV4 => {
                let mut buf = BytesMut::with_capacity(6);
                buf.resize(6, 0);
                let _ = stream.read_exact(&mut buf).await?;

                let mut cursor = buf.to_bytes();
                let v4addr = Ipv4Addr::new(cursor.get_u8(), cursor.get_u8(), cursor.get_u8(), cursor.get_u8());
                let port = cursor.get_u16();
                Ok(Address::SocketAddress(SocketAddr::V4(SocketAddrV4::new(v4addr, port))))
            }
            consts::SOCKS5_ADDR_TYPE_IPV6 => {
                let mut buf = [0u8; 18];
                let _ = stream.read_exact(&mut buf).await?;

                let mut cursor = Cursor::new(&buf);
                let v6addr = Ipv6Addr::new(
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                    cursor.get_u16(),
                );
                let port = cursor.get_u16();

                Ok(Address::SocketAddress(SocketAddr::V6(SocketAddrV6::new(
                    v6addr, port, 0, 0,
                ))))
            }
            consts::SOCKS5_ADDR_TYPE_DOMAIN_NAME => {
                let mut length_buf = [0u8; 1];
                let _ = stream.read_exact(&mut length_buf).await?;
                let length = length_buf[0] as usize;

                // Len(Domain) + Len(Port)
                let buf_length = length + 2;
                let mut buf = BytesMut::with_capacity(buf_length);
                buf.resize(buf_length, 0);
                let _ = stream.read_exact(&mut buf).await?;

                let mut cursor = buf.to_bytes();
                let mut raw_addr = Vec::with_capacity(length);
                raw_addr.put(&mut BufExt::take(&mut cursor, length));
                let addr = match String::from_utf8(raw_addr) {
                    Ok(addr) => addr,
                    Err(..) => return Err(Error::new(Reply::GeneralFailure, "Invalid address encoding")),
                };
                let port = cursor.get_u16();

                Ok(Address::DomainNameAddress(addr, port))
            }
            _ => {
                error!("Invalid address type {}", addr_type);
                Err(Error::new(Reply::AddressTypeNotSupported, "Not supported address type"))
            }
        }
    }

    /// Writes to writer
    #[inline]
    pub async fn write_to<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut buf = BytesMut::with_capacity(self.serialized_len());
        self.write_to_buf(&mut buf);
        writer.write_all(&buf).await
    }

    /// Writes to buffer
    #[inline]
    pub fn write_to_buf<B: BufMut>(&self, buf: &mut B) {
        write_address(self, buf)
    }

    #[inline]
    pub fn serialized_len(&self) -> usize {
        get_addr_len(self)
    }
}

impl Debug for Address {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Address::SocketAddress(ref addr) => write!(f, "{}", addr),
            Address::DomainNameAddress(ref addr, ref port) => write!(f, "{}:{}", addr, port),
        }
    }
}

impl fmt::Display for Address {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Address::SocketAddress(ref addr) => write!(f, "{}", addr),
            Address::DomainNameAddress(ref addr, ref port) => write!(f, "{}:{}", addr, port),
        }
    }
}

impl ToSocketAddrs for Address {
    type Iter = vec::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        match self.clone() {
            Address::SocketAddress(addr) => Ok(vec![addr].into_iter()),
            Address::DomainNameAddress(addr, port) => (&addr[..], port).to_socket_addrs(),
        }
    }
}

impl From<SocketAddr> for Address {
    fn from(s: SocketAddr) -> Address {
        Address::SocketAddress(s)
    }
}

impl From<(String, u16)> for Address {
    fn from((dn, port): (String, u16)) -> Address {
        Address::DomainNameAddress(dn, port)
    }
}

/// Parse `Address` error
#[derive(Debug)]
pub struct AddressError;

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Address, AddressError> {
        match s.parse::<SocketAddr>() {
            Ok(addr) => Ok(Address::SocketAddress(addr)),
            Err(..) => {
                let mut sp = s.split(':');
                match (sp.next(), sp.next()) {
                    (Some(dn), Some(port)) => match port.parse::<u16>() {
                        Ok(port) => Ok(Address::DomainNameAddress(dn.to_owned(), port)),
                        Err(..) => Err(AddressError),
                    },
                    _ => Err(AddressError),
                }
            }
        }
    }
}

fn write_ipv4_address<B: BufMut>(addr: &SocketAddrV4, buf: &mut B) {
    buf.put_u8(consts::SOCKS5_ADDR_TYPE_IPV4); // Address type
    buf.put_slice(&addr.ip().octets()); // Ipv4 bytes
    buf.put_u16(addr.port()); // Port
}

fn write_ipv6_address<B: BufMut>(addr: &SocketAddrV6, buf: &mut B) {
    buf.put_u8(consts::SOCKS5_ADDR_TYPE_IPV6); // Address type
    for seg in &addr.ip().segments() {
        buf.put_u16(*seg); // Ipv6 bytes
    }
    buf.put_u16(addr.port()); // Port
}

fn write_domain_name_address<B: BufMut>(dnaddr: &str, port: u16, buf: &mut B) {
    assert!(dnaddr.len() <= u8::max_value() as usize);

    buf.put_u8(consts::SOCKS5_ADDR_TYPE_DOMAIN_NAME);
    buf.put_u8(dnaddr.len() as u8);
    buf.put_slice(dnaddr[..].as_bytes());
    buf.put_u16(port);
}

fn write_socket_address<B: BufMut>(addr: &SocketAddr, buf: &mut B) {
    match *addr {
        SocketAddr::V4(ref addr) => write_ipv4_address(addr, buf),
        SocketAddr::V6(ref addr) => write_ipv6_address(addr, buf),
    }
}

fn write_address<B: BufMut>(addr: &Address, buf: &mut B) {
    match *addr {
        Address::SocketAddress(ref addr) => write_socket_address(addr, buf),
        Address::DomainNameAddress(ref dnaddr, ref port) => write_domain_name_address(dnaddr, *port, buf),
    }
}

#[inline]
fn get_addr_len(atyp: &Address) -> usize {
    match *atyp {
        Address::SocketAddress(SocketAddr::V4(..)) => 1 + 4 + 2,
        Address::SocketAddress(SocketAddr::V6(..)) => 1 + 8 * 2 + 2,
        Address::DomainNameAddress(ref dmname, _) => 1 + 1 + dmname.len() + 2,
    }
}
