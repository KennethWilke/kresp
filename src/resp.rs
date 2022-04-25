use anyhow::{anyhow, Result};
use bstr::{ByteSlice, ByteVec};

/// Enum for RESP types
#[derive(PartialEq, Debug)]
pub enum RespType {
    /// Represents a simple utf8 string, that should not contain \r or \n characters
    SimpleString(String),
    /// Represents an error, should not contain \r or \n characters
    Error(String),
    /// Represents a signed integer in the 64-bit range
    Integer(i64),
    /// Binary safe string
    BulkString(Vec<u8>),
    /// RESP array type, can be nested
    Array(Vec<RespType>),
    /// Null type, technically a null BulkString
    Null,
    /// Null type that is also an array type, but is not an empty array (wat?)
    NullArray,
}

impl<'a> RespType {
    /// Encodes the RESP type
    pub fn as_bytes(self) -> Vec<u8> {
        use RespType::*;
        let mut bytes = Vec::new();
        match self {
            SimpleString(string) => {
                bytes.push_str("+");
                bytes.push_str(string);
                bytes.push_str("\r\n")
            }
            Error(string) => {
                bytes.push_char('-');
                bytes.push_str(string);
                bytes.push_str("\r\n")
            }
            Integer(string) => {
                bytes.push_char(':');
                bytes.push_str(string.to_string());
                bytes.push_str("\r\n")
            }
            BulkString(string) => {
                bytes.push_char('$');
                bytes.push_str(string.len().to_string());
                bytes.push_str("\r\n");
                bytes.push_str(string);
                bytes.push_str("\r\n")
            }
            Array(array) => {
                bytes.push_char('*');
                bytes.push_str(array.len().to_string());
                bytes.push_str("\r\n");
                for i in array {
                    bytes.push_str(i.as_bytes())
                }
            }
            Null => bytes.push_str("$-1\r\n"),
            NullArray => bytes.push_str("*-1\r\n"),
        };
        bytes
    }

    /// Create a new SimpleString variant
    pub fn simple_string(string: String) -> Result<Self> {
        if string.contains('\r') || string.contains('\n') {
            Err(anyhow!("Simple string contains \\r or \\n"))
        } else {
            Ok(RespType::SimpleString(string))
        }
    }

    /// Create a new Error variant
    pub fn error(string: String) -> Result<Self> {
        if string.contains('\r') || string.contains('\n') {
            Err(anyhow!("Error type contains \\r or \\n"))
        } else {
            Ok(RespType::Error(string))
        }
    }

    /// Create a new integer variant
    pub fn integer(int: i64) -> Self {
        RespType::Integer(int)
    }

    /// Create a new bulk string
    pub fn bulk_string(string: Vec<u8>) -> Self {
        RespType::BulkString(string)
    }

    /// Create a new array
    pub fn array(array: Vec<RespType>) -> Self {
        RespType::Array(array)
    }

    /// Helper for creating an array of bulk strings (such as used for redis commands)
    pub fn command(command: Vec<Vec<u8>>) -> Self {
        let mut cmd = Vec::new();
        for i in command {
            cmd.push(RespType::bulk_string(i.as_bytes().to_vec()))
        }
        RespType::array(cmd)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn assert_expected_encode(resp: RespType, expected: &str) {
        let encoded = resp.as_bytes();
        assert_eq!(expected.as_bytes(), encoded);
    }

    #[test]
    fn simple_string() -> Result<()> {
        let input = "test";
        let resp = RespType::simple_string(input.into())?;
        assert_expected_encode(resp, "+test\r\n");
        Ok(())
    }

    #[test]
    fn error() -> Result<()> {
        let input = "error";
        let resp = RespType::error(input.into())?;
        assert_expected_encode(resp, "-error\r\n");
        Ok(())
    }

    #[test]
    fn integer() {
        let resp = RespType::integer(42);
        assert_expected_encode(resp, ":42\r\n");
    }

    #[test]
    fn bulk_string() {
        let input = "test";
        let resp = RespType::bulk_string(input.into());
        assert_expected_encode(resp, "$4\r\ntest\r\n");
    }

    #[test]
    fn empty_bulk_string() {
        let resp = RespType::bulk_string("".into());
        assert_expected_encode(resp, "$0\r\n\r\n");
    }

    #[test]
    fn null_bulk_string() {
        let resp = RespType::Null;
        assert_expected_encode(resp, "$-1\r\n");
    }

    #[test]
    fn array() {
        let resp = RespType::Array(vec![RespType::SimpleString("test!".into())]);
        assert_expected_encode(resp, "*1\r\n+test!\r\n");
    }

    #[test]
    fn empty_array() {
        let resp = RespType::Array(Vec::new());
        assert_expected_encode(resp, "*0\r\n");
    }

    #[test]
    fn null_array() {
        let resp = RespType::NullArray;
        assert_expected_encode(resp, "*-1\r\n");
    }
}
