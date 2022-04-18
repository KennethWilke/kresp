# kRESP (Kenneth's RESP Parser)

This is a small, streaming RESP (REdis Serialization Protocol) parser with a focus on simplicity and ergonomics.

I am working on an ergonomic async Redis client library, and built this as a precursor step towards that goal. To work best in an asynchronous settings, this library was designed for streaming from the start. Incomplete buffers can be sent to the `RespParser`, which will internally preserve the buffers and parsing state to minimize re-parsing of incomplete data that could stream over a network connection.

# Basic decoding

Instantiate the parser, and feed it some data! The read method will return `Ok(Vec<RespType>)`, with an vector of the results that were parsed, if any.

```rust
let mut parser = RespParser::default();
let simple = "+OK\r\n";

for item in parser.read(simple.as_bytes())? {
    println!("{:#?}", item);
}
```

The types returned are variants of `RespType`:

```rust
let much_more = b"*5\r\n+hello\r\n-woahnow\r\n:-42\r\n$4\r\nhey!\r\n*-1\r\n";
for item in parser.read(much_more)? {
    if let RespType::Array(array) = item {
        for i in array {
            println!("{:#?}", i);
        }
    }
}
```

Partial reads are allowed, so incomplete resp chunks are fine. The output will be empty until an item is complete or until the default limit of 512MB is reached. The items returned will no longer be owned by the parser.

```rust
let some = b"$9\r\n";
assert_eq!(parser.read(some)?.len(), 0);

let the_rest = b"oh, hello\r\n";
println!("{:#?}", parser.read(the_rest)?);
```

The parser will return errors for protocol violations. When an error occurs, all internal buffers are cleared to allow continued use of the parser without the need for additional intervention.

```rust
let bad = b"forgot my type!";
println!("{:#?}", parser.read(bad));

let recovered = b"+all good now though\r\n";
println!("{:#?}", parser.read(recovered)?);
```

# Encoding data

This library also supports encoding `RespType` variants to heap-allocated bytes (`Vec<u8>`).

```rust
let hello = RespType::BulkString("hello".into());
let world = RespType::BulkString("world!".into());
let array = RespType::array(vec![hello, world]);
let encoded = array.as_bytes();
println!("{:#?}", std::str::from_utf8(&encoded)?);
```
