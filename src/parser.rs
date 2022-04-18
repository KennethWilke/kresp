use anyhow::Result;
use thiserror::Error;

use super::buffer::*;
use super::RespConfig;
use super::RespType;

/// Error enumeration used when a parsing error occurs
#[derive(Error, Debug)]
pub enum ParserError {
    /// Error occured when reading a simple string that should end in \r\n
    #[error("Invalid RESP line read: {0}")]
    ReadlineError(String),

    /// Error when an invalid size is read
    #[error("Invalid RESP size: {0}")]
    ReadsizeError(i64),

    /// An internal state machine error, this should not happen, please report
    /// if it does!
    #[error("State error: {0}")]
    StateError(String),

    /// Next byte read was not a type token
    #[error("Invalid RESP type token: {0:#?}")]
    TypeTokenError(char),

    /// Size limit hit
    #[error("RESP size exceeded")]
    SizeExceededError,
}

/// The parser itself, use [`RespParser::read`] to provide it buffers to parse
pub struct RespParser {
    buffer: Vec<u8>,
    state: Option<Box<State>>,
    /// Configuration structure for memory limits
    pub config: RespConfig,
}

#[derive(Debug)]
enum State {
    GetType {
        cursor: usize,
    },
    Simple {
        cursor: usize,
        start: usize,
        simple_type: SimpleType,
    },
    BulkString {
        cursor: usize,
        start: usize,
        size: Option<usize>,
    },
    Array {
        cursor: usize,
        start: usize,
        size: Option<usize>,
        elements: Option<Vec<RespType>>,
        substate: Option<Box<State>>,
    },
}

impl State {
    fn boxed(self) -> Box<State> {
        Box::new(self)
    }

    fn get_type(cursor: usize) -> Box<State> {
        Box::new(State::GetType { cursor })
    }

    fn get_simple(cursor: usize, simple_type: SimpleType) -> Box<State> {
        Box::new(State::Simple {
            cursor,
            start: cursor,
            simple_type,
        })
    }

    fn get_bulk_string(cursor: usize) -> Box<State> {
        Box::new(State::BulkString {
            cursor,
            start: cursor,
            size: None,
        })
    }

    fn get_array(cursor: usize) -> Box<State> {
        Box::new(State::Array {
            cursor,
            start: cursor,
            size: None,
            elements: None,
            substate: None,
        })
    }
}

#[derive(Debug)]
enum StateResult {
    Incomplete(Box<State>),
    Done(RespType, usize),
}

#[derive(Debug)]
enum SimpleType {
    String,
    Error,
    Integer,
}

impl Default for RespParser {
    fn default() -> Self {
        Self::new(RespConfig::default())
    }
}

impl RespParser {
    /// Creates a new instance, can use [`RespParser`.`default`] for common setups
    pub fn new(config: RespConfig) -> Self {
        RespParser {
            buffer: Vec::new(),
            state: None,
            config,
        }
    }

    /// Copy and parses the provided buffer, returns a list of [`RespType`] variant results
    pub fn read(&mut self, buffer: &[u8]) -> Result<Vec<RespType>> {
        for byte in buffer {
            self.buffer.push(*byte);
        }

        if self.buffer.len() > self.config.max_buffer_size {
            self.buffer.clear();
            return Err(ParserError::SizeExceededError.into());
        }

        let mut items = Vec::new();
        if let Some(state) = self.state.take() {
            match self.process_state(state) {
                Ok(result) => match result {
                    StateResult::Incomplete(state) => {
                        self.state = Some(state.boxed());
                        return Ok(items)
                    }
                    StateResult::Done(item, end) => {
                        self.buffer.drain(..end);
                        items.push(item)
                    }
                }
                Err(error) => {
                    self.buffer.clear();
                    return Err(error)
                }
            }
        }

        loop {
            match self.get_next() {
                Ok(result) => match result {
                    Some(item) => items.push(item),
                    None => return Ok(items),
                },
                Err(error) => {
                    self.buffer.clear();
                    return Err(error)
                }
            }
        }
    }

    fn get_next(&mut self) -> Result<Option<RespType>> {
        match self.get_type(State::get_type(0))? {
            StateResult::Incomplete(state) => {
                self.state = Some(state.boxed());
                Ok(None)
            }
            StateResult::Done(item, end) => {
                self.buffer.drain(..end);
                Ok(Some(item))
            }
        }
    }

    fn process_state(&self, state: Box<State>) -> Result<StateResult> {
        match *state {
            State::GetType { .. } => self.get_type(state),
            State::Simple { .. } => self.get_simple(state),
            State::BulkString { .. } => self.get_bulk_string(state),
            State::Array { .. } => self.get_array(state),
        }
    }

    fn get_type(&self, state: Box<State>) -> Result<StateResult> {
        if let State::GetType { cursor } = *state {
            if self.buffer.len() <= cursor {
                return Ok(StateResult::Incomplete(State::get_type(cursor)));
            }

            let next_cursor = cursor + 1;
            let state = match &self.buffer[cursor] {
                b'+' => State::get_simple(next_cursor, SimpleType::String),
                b'-' => State::get_simple(next_cursor, SimpleType::Error),
                b':' => State::get_simple(next_cursor, SimpleType::Integer),
                b'$' => State::get_bulk_string(next_cursor),
                b'*' => State::get_array(next_cursor),
                other => return Err(ParserError::TypeTokenError(*other as char).into()),
            };

            if self.buffer.len() > cursor + 1 {
                self.process_state(state)
            } else {
                Ok(StateResult::Incomplete(state.boxed()))
            }
        } else {
            Err(ParserError::StateError(format!(
                "get_type received wrong state type: {:#?}",
                state
            ))
            .into())
        }
    }

    fn get_simple(&self, state: Box<State>) -> Result<StateResult> {
        if let State::Simple {
            cursor,
            start,
            simple_type,
        } = *state
        {
            match readline(&self.buffer, cursor, start)? {
                ReadlineResult::Line { line, cursor } => {
                    if line.len() > self.config.max_resp_size {
                        return Err(ParserError::SizeExceededError.into());
                    }
                    let result = match simple_type {
                        SimpleType::String => RespType::SimpleString(line),
                        SimpleType::Error => RespType::Error(line),
                        SimpleType::Integer => RespType::Integer(line.parse()?),
                    };
                    Ok(StateResult::Done(result, cursor))
                }
                ReadlineResult::None { cursor } => Ok(StateResult::Incomplete(
                    State::Simple {
                        cursor,
                        start,
                        simple_type,
                    }
                    .boxed(),
                )),
            }
        } else {
            Err(ParserError::StateError(format!(
                "get_simple received wrong state type: {:#?}",
                state
            ))
            .into())
        }
    }

    fn get_bulk_string(&self, state: Box<State>) -> Result<StateResult> {
        if let State::BulkString {
            cursor,
            start,
            size: string_length,
        } = *state
        {
            let (cursor, size) = match string_length {
                None => match readsize(&self.buffer, cursor, start)? {
                    ReadsizeResult::None(cursor) => {
                        let state = State::BulkString {
                            cursor,
                            start,
                            size: None,
                        };
                        return Ok(StateResult::Incomplete(state.boxed()));
                    }
                    ReadsizeResult::Null(cursor) => {
                        let result = RespType::Null;
                        return Ok(StateResult::Done(result, cursor));
                    }
                    ReadsizeResult::Size { end, size } => (end, size),
                },
                Some(size) => (cursor, size),
            };
            if size > self.config.max_resp_size {
                return Err(ParserError::SizeExceededError.into());
            }

            match readbuffer(&self.buffer, cursor, size) {
                Some((vector, end)) => {
                    let result = RespType::BulkString(vector);
                    Ok(StateResult::Done(result, end))
                }
                None => {
                    let state = State::BulkString {
                        cursor,
                        start,
                        size: Some(size),
                    }
                    .boxed();
                    Ok(StateResult::Incomplete(state))
                }
            }
        } else {
            Err(ParserError::StateError(format!(
                "get_bulk_string received wrong state type: {:#?}",
                state
            ))
            .into())
        }
    }

    fn get_array(&self, state: Box<State>) -> Result<StateResult> {
        if let State::Array {
            cursor,
            start,
            size: array_size,
            elements,
            mut substate,
        } = *state
        {
            let (cursor, size) = match array_size {
                Some(size) => (cursor, size),
                None => match readsize(&self.buffer, cursor, start)? {
                    ReadsizeResult::None(cursor) => {
                        let state = State::Array {
                            cursor,
                            start,
                            size: None,
                            elements: None,
                            substate: None,
                        };
                        return Ok(StateResult::Incomplete(state.boxed()));
                    }
                    ReadsizeResult::Null(cursor) => {
                        let result = RespType::NullArray;
                        return Ok(StateResult::Done(result, cursor));
                    }
                    ReadsizeResult::Size { end, size } => {
                        if size == 0 {
                            let result = RespType::Array(Vec::new());
                            return Ok(StateResult::Done(result, end));
                        } else {
                            (end, size)
                        }
                    }
                },
            };
            if size > self.config.max_resp_size {
                return Err(ParserError::SizeExceededError.into());
            }

            let mut elements = match elements {
                Some(elements) => elements,
                None => Vec::new(),
            };
            let mut cursor = cursor;
            while elements.len() < size {
                let state = match substate {
                    Some(_) => substate.take().unwrap(),
                    None => State::get_type(cursor),
                };
                match self.process_state(state)? {
                    StateResult::Done(result, end) => {
                        cursor = end;
                        elements.push(result);
                    }
                    StateResult::Incomplete(substate) => {
                        let state = State::Array {
                            cursor,
                            start,
                            size: Some(size),
                            elements: Some(elements),
                            substate: Some(substate),
                        };
                        return Ok(StateResult::Incomplete(state.boxed()));
                    }
                }
            }
            let result = RespType::Array(elements);
            Ok(StateResult::Done(result, cursor))
        } else {
            Err(ParserError::StateError(format!(
                "get_array received wrong state type: {:#?}",
                state
            ))
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RespType::*;

    fn test_parser_ok<'a, T>(buffer: T) -> Vec<RespType>
    where
        &'a [u8]: From<T>,
    {
        let mut parser = RespParser::default();
        match parser.read(buffer.into()) {
            Ok(results) => results,
            other => panic!("result was not Ok(), was {:#?}", other),
        }
    }

    fn test_parser_err<'a, T>(buffer: T)
    where
        &'a [u8]: From<T>,
    {
        let mut parser = RespParser::default();
        let result = parser.read(buffer.into());
        assert!(result.is_err());
    }

    fn assert_empty_result(results: Vec<RespType>) {
        let result_length = results.len();
        assert_eq!(
            result_length, 0,
            "result was not empty, contained {} elements",
            result_length
        );
    }

    fn assert_num_results(results: &Vec<RespType>, expected: usize) {
        let result_length = results.len();
        assert_eq!(
            result_length, expected,
            "result was of unexpected length, contained {} elements, expected {}",
            result_length, expected
        )
    }

    #[test]
    fn empty_start() {
        let results = test_parser_ok(b"");

        assert_empty_result(results);
    }

    #[test]
    fn complex_nested() {
        let results = test_parser_ok(b"*3\r\n*-1\r\n*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n*5\r\n+test\r\n-test3\r\n:-12345\r\n$2\r\nab\r\n$-1\r\n");

        assert_num_results(&results, 1);
        if let Array(array) = &results[0] {
            assert_eq!(array.len(), 3);
            assert_eq!(array[0], RespType::NullArray);
            if let Array(nested) = &array[1] {
                assert_eq!(nested.len(), 2);
                assert_eq!(nested[0], BulkString("hello".into()));
                assert_eq!(nested[1], BulkString("world".into()));
            } else {
                panic!("Nested array at pos 1 expected")
            }
            if let Array(mixed) = &array[2] {
                assert_eq!(mixed.len(), 5);
                assert_eq!(mixed[0], SimpleString("test".into()));
                assert_eq!(mixed[1], Error("test3".into()));
                assert_eq!(mixed[2], Integer(-12345));
                assert_eq!(mixed[3], BulkString("ab".into()));
                assert_eq!(mixed[4], Null);
            } else {
                panic!("Mixed array at pos 2 expected")
            }
        } else {
            panic!("Array type expected")
        }
    }

    #[test]
    fn complex_nested_onebyte() -> Result<()> {
        let mut parser = RespParser::default();
        for byte in b"*3\r\n*-1\r\n*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n*5\r\n+test\r\n-test3\r\n:-12345\r\n$2\r\nab\r\n$-1\r" {
            let results = parser.read(&[*byte])?;
            assert_eq!(results.len(), 0);
        }

        let results = parser.read(b"\n")?;

        assert_num_results(&results, 1);
        if let Array(array) = &results[0] {
            assert_eq!(array.len(), 3);
            assert_eq!(array[0], RespType::NullArray);
            if let Array(nested) = &array[1] {
                assert_eq!(nested.len(), 2);
                assert_eq!(nested[0], BulkString("hello".into()));
                assert_eq!(nested[1], BulkString("world".into()));
            } else {
                panic!("Nested array at pos 1 expected")
            }
            if let Array(mixed) = &array[2] {
                assert_eq!(mixed.len(), 5);
                assert_eq!(mixed[0], SimpleString("test".into()));
                assert_eq!(mixed[1], Error("test3".into()));
                assert_eq!(mixed[2], Integer(-12345));
                assert_eq!(mixed[3], BulkString("ab".into()));
                assert_eq!(mixed[4], Null);
                Ok(())
            } else {
                panic!("Mixed array at pos 2 expected")
            }
        } else {
            panic!("Array type expected")
        }
    }

    mod simple_string {
        use super::*;

        fn assert_simple_string(elements: &Vec<RespType>, index: usize, expected: &str) {
            let element = &elements.get(index);
            assert!(element.is_some());

            match element.unwrap() {
                SimpleString(string) => {
                    assert_eq!(string, expected);
                }
                _ => {
                    panic!("Expected SimpleString variant")
                }
            };
        }

        #[test]
        fn valid() {
            let results = test_parser_ok(b"+Valid!\r\n");

            assert_num_results(&results, 1);
            assert_simple_string(&results, 0, "Valid!");
        }

        #[test]
        fn valid_remainder() {
            let results = test_parser_ok(b"+valid and then some\r\n+");

            assert_num_results(&results, 1);
            assert_simple_string(&results, 0, "valid and then some");
        }

        #[test]
        fn valid_incomplete() {
            let results = test_parser_ok(b"+OK\r");

            assert_empty_result(results);
        }

        #[test]
        fn invalid_char_after_cr() {
            test_parser_err(b"+OK\rx");
        }

        #[test]
        fn invalid_newline() {
            test_parser_err(b"+OK\n\r\n");
        }
    }

    mod error {
        use super::*;

        fn assert_error(elements: &Vec<RespType>, index: usize, expected: &str) {
            let element = &elements.get(index);
            assert!(element.is_some());

            match element.unwrap() {
                Error(string) => {
                    assert_eq!(string, expected);
                }
                _ => {
                    panic!("Expected Error variant")
                }
            };
        }

        #[test]
        fn valid() {
            let results = test_parser_ok(b"-Valid!\r\n");

            assert_num_results(&results, 1);
            assert_error(&results, 0, "Valid!");
        }

        #[test]
        fn remainder() {
            let results = test_parser_ok(b"-Valid!\r\n:");

            assert_num_results(&results, 1);
            assert_error(&results, 0, "Valid!");
        }

        #[test]
        fn two() {
            let results = test_parser_ok(b"-Valid!\r\n-andmore\r\n");

            assert_num_results(&results, 2);
            assert_error(&results, 0, "Valid!");
            assert_error(&results, 1, "andmore");
        }
    }

    mod integer {
        use super::*;

        fn assert_integer(elements: &Vec<RespType>, index: usize, expected: i64) {
            let element = &elements.get(index);
            assert!(element.is_some());

            match element.unwrap() {
                Integer(int) => {
                    assert_eq!(*int, expected);
                }
                _ => {
                    panic!("Expected Integer variant")
                }
            };
        }

        #[test]
        fn valid() {
            let results = test_parser_ok(b":1234\r\n");

            assert_num_results(&results, 1);
            assert_integer(&results, 0, 1234);
        }

        #[test]
        fn valid_negative() {
            let results = test_parser_ok(b":-1234\r\n");

            assert_num_results(&results, 1);
            assert_integer(&results, 0, -1234);
        }

        #[test]
        fn invalid() {
            test_parser_err(b":hi\r\n");
        }
    }

    mod bulk_string {
        use super::*;

        fn assert_bulk_string(results: &Vec<RespType>, index: usize, expected: &[u8]) {
            let element = &results.get(index);
            assert!(element.is_some());

            match element.unwrap() {
                BulkString(string) => {
                    assert_eq!(string, expected);
                }
                _ => {
                    panic!("Expected BulkString variant")
                }
            };
        }

        #[test]
        fn valid() {
            let results = test_parser_ok(b"$6\r\nValid!\r\n");

            assert_num_results(&results, 1);
            assert_bulk_string(&results, 0, "Valid!".as_bytes());
        }

        #[test]
        fn two() {
            let results = test_parser_ok(b"$6\r\nValid!\r\n$5\r\nwooo!\r\n");

            assert_num_results(&results, 2);
            assert_bulk_string(&results, 0, "Valid!".as_bytes());
            assert_bulk_string(&results, 1, "wooo!".as_bytes());
        }

        #[test]
        fn remainder() {
            let results = test_parser_ok(b"$6\r\nValid!\r\n+OK");

            assert_num_results(&results, 1);
            assert_bulk_string(&results, 0, "Valid!".as_bytes());
        }

        #[test]
        fn empty() {
            let results = test_parser_ok(b"$0\r\n\r\n");

            assert_num_results(&results, 1);
        }

        #[test]
        fn null() {
            let results = test_parser_ok(b"$-1\r\n");

            assert_num_results(&results, 1);
        }
    }

    mod array {
        use super::*;

        fn _assert_array_length(array: &RespType, length: usize) {
            match array {
                Array(array) => {
                    assert_eq!(array.len(), length);
                }
                _ => {
                    panic!("Expected Array variant")
                }
            };
        }

        #[test]
        fn start() {
            let results = test_parser_ok(b"*");
            assert_empty_result(results);
        }

        #[test]
        fn hello_world() {
            let results = test_parser_ok(b"*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n");

            assert_num_results(&results, 1);
        }

        #[test]
        fn nested() {
            let results = test_parser_ok(b"*1\r\n*3\r\n$5\r\nhello\r\n+ok\r\n*-1\r\n");
            assert_num_results(&results, 1);
        }

        #[test]
        fn null() -> Result<()> {
            let results = test_parser_ok(b"*-1\r\n");

            assert_num_results(&results, 1);
            match results.first().unwrap() {
                RespType::NullArray => {
                    // good
                }
                _ => {
                    panic!("null array expected");
                }
            };
            Ok(())
        }

        #[test]
        fn empty() {
            let results = test_parser_ok(b"*0\r\n");

            assert_num_results(&results, 1);
        }
    }
}
