use super::ParserError;
use anyhow::Result;
use bstr::ByteSlice;

#[derive(Debug)]
pub enum ReadlineResult {
    Line { line: String, cursor: usize },
    None { cursor: usize },
}

pub fn readline(buffer: &[u8], cursor: usize, start: usize) -> Result<ReadlineResult> {
    match buffer[cursor..].find_byte(b'\r') {
        Some(cr) => {
            let end = cursor + cr;
            let length_needed = end + 2;

            match buffer.len() >= length_needed {
                false => Ok(ReadlineResult::None {
                    cursor: buffer.len() - 1,
                }),
                true => match buffer[length_needed - 1] == b'\n' {
                    true => {
                        let line = buffer[start..end].to_str()?.to_string();
                        if line.contains('\n') {
                            let error = "line contains premature \\n".to_string();
                            return Err(ParserError::ReadlineError(error).into());
                        }
                        Ok(ReadlineResult::Line {
                            line,
                            cursor: length_needed,
                        })
                    }
                    false => {
                        let error = format!("expected '\\n' after '\\r', got {}", buffer[cr + 1]);
                        Err(ParserError::ReadlineError(error).into())
                    }
                },
            }
        }
        None => Ok(ReadlineResult::None {
            cursor: buffer.len(),
        }),
    }
}

pub fn readbuffer(buffer: &[u8], cursor: usize, size: usize) -> Option<(Vec<u8>, usize)> {
    let end = cursor + size + 2;
    if buffer.len() >= end {
        let data = buffer[cursor..cursor + size].into();
        return Some((data, end));
    }
    None
}

#[derive(Debug)]
pub enum ReadsizeResult {
    Size { end: usize, size: usize },
    Null(usize),
    None(usize),
}

pub fn readsize(buffer: &[u8], cursor: usize, start: usize) -> Result<ReadsizeResult> {
    match readline(buffer, cursor, start)? {
        ReadlineResult::Line { line, cursor: end } => {
            let size: i64 = line.parse()?;
            let result = match size {
                invalid if size < -1 => return Err(ParserError::ReadsizeError(invalid).into()),
                -1 => ReadsizeResult::Null(end),
                size => ReadsizeResult::Size {
                    end,
                    size: size.try_into()?,
                },
            };
            Ok(result)
        }
        ReadlineResult::None { cursor: end } => Ok(ReadsizeResult::None(end)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod readline {
        use super::*;

        #[test]
        fn valid() {
            let buffer: Vec<u8> = "hello!\r\n".into();
            let expected = "hello!".to_string();

            match readline(&buffer, 0, 0) {
                Ok(ReadlineResult::Line { line, cursor }) => {
                    assert_eq!(line.len(), 6);
                    assert_eq!(line, expected);
                    assert_eq!(cursor, 8);
                }
                _ => {
                    panic!("valid line was expected")
                }
            }
        }

        #[test]
        fn invalid() {
            let buffer: Vec<u8> = "hello!\rx".into();

            match readline(&buffer, 0, 0) {
                Err(error) => {
                    println!("{:#?}", error);
                }
                _ => {
                    panic!("error was expected")
                }
            }
        }

        #[test]
        fn invalid_newline() {
            let buffer: Vec<u8> = "hel\nlo!\r\n".into();

            match readline(&buffer, 0, 0) {
                Err(error) => {
                    println!("{:#?}", error);
                }
                _ => {
                    panic!("error was expected")
                }
            }
        }

        #[test]
        fn remainder() {
            let buffer: Vec<u8> = "hello!\r\nextra".into();
            let expected = "hello!".to_string();

            match readline(&buffer, 0, 0) {
                Ok(ReadlineResult::Line { line, cursor }) => {
                    assert_eq!(line.len(), 6);
                    assert_eq!(line, expected);
                    assert_eq!(cursor, 8);
                }
                other => {
                    panic!("valid line was expected, got {:#?}", other);
                }
            }
        }

        #[test]
        fn none_progresses_cursor() {
            let buffer: Vec<u8> = "hello!".into();

            match readline(&buffer, 0, 0) {
                Ok(ReadlineResult::None { cursor }) => {
                    assert_eq!(cursor, 6);
                }
                _ => {
                    panic!("Ok(ReadlineResult::None{{}}) variant was not expected")
                }
            }
        }

        mod readsize {
            use super::*;

            #[test]
            fn valid() {
                let buffer: Vec<u8> = "test\r\n".into();
                if let Some((data, end)) = readbuffer(&buffer, 0, 4) {
                    assert_eq!(data, "test".as_bytes().to_vec());
                    assert_eq!(end, 6);
                } else {
                    panic!("readsize was None, expected 'test'")
                }
            }

            #[test]
            fn short() {
                let buffer: Vec<u8> = "test\r".into();
                assert_eq!(readbuffer(&buffer, 0, 4), None);
            }

            #[test]
            fn offset() {
                let buffer: Vec<u8> = "1234test\r\n".into();
                if let Some((data, end)) = readbuffer(&buffer, 4, 4) {
                    assert_eq!(data, "test".as_bytes().to_vec());
                    assert_eq!(end, 10);
                } else {
                    panic!("readsize was None, expected 'test'")
                }
            }
        }

        #[test]
        fn end_on_cr() {
            let buffer: Vec<u8> = "hello!\r".into();

            match readline(&buffer, 0, 0) {
                Ok(ReadlineResult::None { cursor: end }) => {
                    assert_eq!(end, 6);
                }
                _ => {
                    panic!("Ok(ReadlineResult::None{{}}) variant was not expected")
                }
            }
        }

        #[test]
        fn offset() {
            let buffer: Vec<u8> = "123hello!\r\nextra".into();
            let expected = "hello!".to_string();
            let cursor = 3;

            match readline(&buffer, cursor, cursor) {
                Ok(ReadlineResult::Line { line, cursor }) => {
                    assert_eq!(line.len(), 6);
                    assert_eq!(line, expected);
                    assert_eq!(cursor, 11);
                }
                other => {
                    panic!("valid line was expected, got {:#?}", other);
                }
            }
        }

        #[test]
        fn offset_end_on_cr() {
            let buffer: Vec<u8> = "123hello!\r".into();
            let cursor = 3;

            match readline(&buffer, cursor, cursor) {
                Ok(ReadlineResult::None { cursor: end }) => {
                    assert_eq!(end, 9);
                }
                other => {
                    panic!("expected None, got {:#?}", other);
                }
            }
        }

        #[test]
        fn offset_start() {
            let buffer: Vec<u8> = "123hello!\r\nextra".into();
            let expected = "123hello!".to_string();
            let cursor = 3;

            match readline(&buffer, cursor, 0) {
                Ok(ReadlineResult::Line { line, cursor }) => {
                    assert_eq!(line.len(), 9);
                    assert_eq!(line, expected);
                    assert_eq!(cursor, 11);
                }
                other => {
                    panic!("valid line was expected, got {:#?}", other);
                }
            }
        }
    }
}
