use openssl::ssl::SslStream;
use std::net::TcpStream;

pub trait AsRawDesc: std::os::windows::io::AsRawSocket {}

#[derive(Debug)]
pub struct AsyncSslStream {
    s: SslStream<TcpStream>,
}

unsafe impl async_io::IoSafe for AsyncSslStream {}

impl AsyncSslStream {
    pub fn new(s: SslStream<TcpStream>) -> Self {
        Self { s }
    }
}

impl std::os::windows::io::AsRawSocket for AsyncSslStream {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        self.s.get_ref().as_raw_socket()
    }
}

impl std::os::windows::io::AsSocket for AsyncSslStream {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        self.s.get_ref().as_socket()
    }
}

impl AsRawDesc for AsyncSslStream {}

impl std::io::Read for AsyncSslStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.s.read(buf)
    }
}

impl std::io::Write for AsyncSslStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.s.write(buf)
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.s.flush()
    }
}
