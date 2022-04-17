#![doc = include_str!("../README.md")]

mod buffer;
mod config;
mod parser;
mod resp;

pub use config::RespConfig;
pub use parser::{ParserError, RespParser};
pub use resp::RespType;
