Rust Futures Runtime
===

This is my take in implementing single-threaded futures runtime
for Rust for Linux. I did this project for learning `async`/`await`
programming in Rust and `epoll` in Linux.

## Implemented

1. Common types, macros, functions and constants ([`src/types.rs`](src/types.rs)):
   1. `AsyncRead` and `AsyncWrite` traits
   2. `async_write` and `async_writeln` macros
2. Epoll abstraction ([`src/epoll.rs`](src/epoll.rs))
3. Listen socket abstractions ([`src/listen_socket.rs`](src/listen_socket.rs))
4. Futures executor that can wake either on activity
   or by timeout, futures waker ([`src/executor.rs`](src/executor.rs))
5. Asynchronous TCP server ([`src/async_tcp_listener.rs`](src/async_tcp_listener.rs))
6. Asynchronous FD wrapper ([`src/async_file.rs`](src/async_file.rs))
7. Asynchronous buffered reader for
   reading FDs line by line ([`src/async_file.rs`](src/async_file.rs))
8. Asynchronous sleep function ([`src/async_sleep.rs`](src/async_sleep.rs))

## Usage example

See [`src/main.rs`](src/main.rs) for an example TCP server that listens
on port `1234` and has following plain-text interface:

1. `quit`: close the connection
2. `echo $text`: print a `$text` line
3. `sleep $time`: sleep `$time` ms and then print `done sleeping` line

You can use either `nc localhost 1234` or `telnet localhost 1234` to test it.

The server also prints logs to stdout, clients are identified by their FDs.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.
