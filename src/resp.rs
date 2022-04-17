use bstr::{ByteSlice, ByteVec};

#[derive(PartialEq, Debug)]
pub enum RespType {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Vec<u8>),
    Array(Vec<RespType>),
    Null,
    NullArray,
}

impl<'a> RespType {
    pub fn as_bytes(self) -> Vec<u8> {
        use RespType::*;
        let mut bytes = Vec::new();
        match self {
            SimpleString(string) => {
                bytes.push_str("+");
                bytes.push_str(string)
            }
            Error(string) => {
                bytes.push_char('-');
                bytes.push_str(string)
            }
            Integer(string) => {
                bytes.push_char(':');
                bytes.push_str(string.to_string())
            }
            BulkString(string) => {
                bytes.push_char('$');
                bytes.push_str(string.len().to_string());
                bytes.push_str("\r\n");
                bytes.push_str(string)
            }
            Array(array) => {
                bytes.push_char('*');
                bytes.push_str(array.len().to_string());
                bytes.push_str("\r\n");
                for i in array {
                    bytes.push_str(i.as_bytes())
                }
            }
            Null => bytes.push_str("$-1"),
            NullArray => bytes.push_str("*-1"),
        };
        bytes.push_str(b"\r\n");
        bytes
    }

    pub fn simple_string(string: String) -> Self {
        RespType::SimpleString(string)
    }

    #[allow(dead_code)]
    pub fn error(string: String) -> Self {
        RespType::Error(string)
    }

    pub fn integer(int: i64) -> Self {
        RespType::Integer(int)
    }

    pub fn bulk_string(string: Vec<u8>) -> Self {
        RespType::BulkString(string)
    }

    pub fn array(array: Vec<RespType>) -> Self {
        RespType::Array(array)
    }

    pub fn command(command: Vec<Vec<u8>>) -> Self {
        let mut cmd = Vec::new();
        for i in command {
            cmd.push(RespType::bulk_string(i.as_bytes().to_vec()))
        }
        RespType::array(cmd)
    }
}
