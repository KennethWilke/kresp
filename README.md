# kRESP (Kenneth's RESP Parser)

This is a small, streaming RESP (REdis Serialization Protocol) parser with a focus on simplicity and ergonomics.

I am working on an ergonomic async Redis client library, and built this as a precursor step towards that goal. To work best in an asynchronous settings, this library was designed for streaming from the start. Incomplete buffers can be sent to the `RespParser`, which will internally preserve the buffers and parsing state to minimize re-parsing of incomplete data that could stream over a network connection.

