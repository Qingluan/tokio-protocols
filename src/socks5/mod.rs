
use tokio::prelude::*;
use tokio::net::{self,TcpStream};
use crate::utils::Except;
use crate::utils::err_with;
use crate::utils::status;
use byteorder::{NetworkEndian};
use byteorder::ByteOrder;
use std::str;

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
