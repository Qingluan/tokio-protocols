use std::io::{Error as IoError, ErrorKind};
pub use colored::Colorize; 
pub type Except<T> = Result<T, Box<dyn std::error::Error>>;
pub fn err_with(msg:&str) -> Box<dyn std::error::Error  >{
    let msg:&'static str = Box::leak(msg.to_string().into_boxed_str());
    Box::new(IoError::new(ErrorKind::NotFound, msg))
}

pub fn status(label: &str, stateus:bool){
    if stateus {
        print!("{} [{}]\r",label.trim(),"ok".green().bold());
    }else{
        println!("{} [{}]",label,"no".yellow().bold());    
    }
}