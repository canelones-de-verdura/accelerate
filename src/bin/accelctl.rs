use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

const SOCKET_PATH: &str = "/run/accel.socket";

fn main() {
    let mut socket = UnixStream::connect(SOCKET_PATH).unwrap();
    loop {
        socket.write(b"Hola!").unwrap();
    }
}
