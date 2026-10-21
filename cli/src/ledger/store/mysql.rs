//! A small synchronous MySQL client for the local Dolt server (ADR 0018).
//!
//! It speaks the text protocol over a Unix socket and nothing else: the handshake with
//! the empty password of the local `root` user, `COM_QUERY`, and the result sets that
//! come back. There is no TLS, no prepared statement and no compression. Dolt is the only
//! server it talks to. A crate would bring TLS and regex trees that this path never uses
//! (see the ADR for the crates that were tried).
//!
//! Every value is converted to the JSON shape that `dolt sql -r json` prints, so the code
//! that reads rows does not know which path produced them.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Number, Value};

const CLIENT_LONG_PASSWORD: u32 = 0x0000_0001;
const CLIENT_LONG_FLAG: u32 = 0x0000_0004;
const CLIENT_CONNECT_WITH_DB: u32 = 0x0000_0008;
const CLIENT_PROTOCOL_41: u32 = 0x0000_0200;
const CLIENT_TRANSACTIONS: u32 = 0x0000_2000;
const CLIENT_SECURE_CONNECTION: u32 = 0x0000_8000;
const CLIENT_MULTI_STATEMENTS: u32 = 0x0001_0000;
const CLIENT_MULTI_RESULTS: u32 = 0x0002_0000;
const CLIENT_PLUGIN_AUTH: u32 = 0x0008_0000;

const SERVER_MORE_RESULTS_EXISTS: u16 = 0x0008;
const UNSIGNED_FLAG: u16 = 0x0020;
const MAX_PACKET: usize = 0x00ff_ffff;
const UTF8MB4_GENERAL_CI: u8 = 45;

const TYPE_DECIMAL: u8 = 0x00;
const TYPE_TINY: u8 = 0x01;
const TYPE_SHORT: u8 = 0x02;
const TYPE_LONG: u8 = 0x03;
const TYPE_FLOAT: u8 = 0x04;
const TYPE_DOUBLE: u8 = 0x05;
const TYPE_LONGLONG: u8 = 0x08;
const TYPE_INT24: u8 = 0x09;
const TYPE_YEAR: u8 = 0x0d;
const TYPE_JSON: u8 = 0xf5;
const TYPE_NEWDECIMAL: u8 = 0xf6;

/// Why a call failed. The caller decides what a failure of each kind means.
#[derive(Debug)]
pub enum Error {
    /// The server answered with an error packet: the statement is wrong or cannot apply.
    Server {
        #[allow(dead_code)]
        code: u16,
        message: String,
    },
    /// The socket failed or the server went silent for longer than the timeout.
    Io(std::io::Error),
    /// The server sent bytes that this client does not understand.
    Protocol(String),
}

impl Error {
    /// True when the connection is unusable and the server may be gone or hung.
    pub fn is_connection(&self) -> bool {
        matches!(self, Error::Io(_) | Error::Protocol(_))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Server { message, .. } => write!(f, "{}", message),
            Error::Io(e) => write!(f, "{}", e),
            Error::Protocol(m) => write!(f, "{}", m),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

struct Column {
    name: String,
    kind: u8,
    unsigned: bool,
}

/// One result set. A statement that returns no rows gives one with no rows.
#[derive(Debug, Default)]
pub struct ResultSet {
    pub rows: Vec<Value>,
}

pub struct Conn {
    stream: UnixStream,
    seq: u8,
}

impl Conn {
    /// Connect to the socket as `root` with no password and select `database`. `timeout`
    /// bounds the connect, the handshake and every later read and write.
    pub fn connect(
        socket: &Path,
        database: &str,
        timeout: Option<Duration>,
    ) -> Result<Conn, Error> {
        let stream = UnixStream::connect(socket)?;
        no_sigpipe(&stream)?;
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        let mut conn = Conn { stream, seq: 0 };
        conn.handshake(database)?;
        Ok(conn)
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.stream.set_read_timeout(timeout)?;
        self.stream.set_write_timeout(timeout)?;
        Ok(())
    }

    fn handshake(&mut self, database: &str) -> Result<(), Error> {
        let greeting = self.read_packet()?;
        if greeting.first() == Some(&0xff) {
            return Err(parse_error(&greeting));
        }
        if greeting.first() != Some(&10) {
            return Err(Error::Protocol(format!(
                "unexpected greeting from the Dolt server (protocol {:?})",
                greeting.first()
            )));
        }
        let flags = CLIENT_LONG_PASSWORD
            | CLIENT_LONG_FLAG
            | CLIENT_CONNECT_WITH_DB
            | CLIENT_PROTOCOL_41
            | CLIENT_TRANSACTIONS
            | CLIENT_SECURE_CONNECTION
            | CLIENT_MULTI_STATEMENTS
            | CLIENT_MULTI_RESULTS
            | CLIENT_PLUGIN_AUTH;
        let mut response = Vec::new();
        response.extend_from_slice(&flags.to_le_bytes());
        response.extend_from_slice(&(MAX_PACKET as u32).to_le_bytes());
        response.push(UTF8MB4_GENERAL_CI);
        response.extend_from_slice(&[0u8; 23]);
        response.extend_from_slice(b"root\0");
        // The local root user has no password: an empty authentication response.
        response.push(0);
        response.extend_from_slice(database.as_bytes());
        response.push(0);
        response.extend_from_slice(b"mysql_native_password\0");
        self.write_packet(&response)?;
        loop {
            let reply = self.read_packet()?;
            match reply.first() {
                Some(0x00) => return Ok(()),
                Some(0xff) => return Err(parse_error(&reply)),
                // A request to switch plugin: an empty password answers every plugin
                // that takes one.
                Some(0xfe) => self.write_packet(&[])?,
                other => {
                    return Err(Error::Protocol(format!(
                        "unexpected authentication reply from the Dolt server ({:?})",
                        other
                    )))
                }
            }
        }
    }

    /// Run `sql`, which may hold several statements, and return the result set of each.
    /// The first failing statement stops the call and is the error.
    pub fn query(&mut self, sql: &str) -> Result<Vec<ResultSet>, Error> {
        self.seq = 0;
        let mut packet = Vec::with_capacity(sql.len() + 1);
        packet.push(0x03);
        packet.extend_from_slice(sql.as_bytes());
        self.write_packet(&packet)?;
        let mut sets = Vec::new();
        loop {
            let first = self.read_packet()?;
            let more;
            match first.first() {
                Some(0xff) => {
                    // Drain nothing: the server sends nothing after an error packet.
                    return Err(parse_error(&first));
                }
                Some(0x00) => {
                    let status = ok_status(&first)?;
                    more = status & SERVER_MORE_RESULTS_EXISTS != 0;
                    sets.push(ResultSet::default());
                }
                Some(0xfb) => {
                    return Err(Error::Protocol(
                        "the Dolt server asked for a local file".to_string(),
                    ))
                }
                Some(_) => {
                    let (count, _) = read_lenenc(&first, 0)?;
                    let mut columns = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let def = self.read_packet()?;
                        columns.push(parse_column(&def)?);
                    }
                    let eof = self.read_packet()?;
                    if eof.first() != Some(&0xfe) {
                        return Err(Error::Protocol(
                            "expected the end of the column definitions".to_string(),
                        ));
                    }
                    let mut rows = Vec::new();
                    loop {
                        let row = self.read_packet()?;
                        match row.first() {
                            Some(0xfe) if row.len() < 9 => {
                                let status = eof_status(&row);
                                more = status & SERVER_MORE_RESULTS_EXISTS != 0;
                                break;
                            }
                            Some(0xff) => return Err(parse_error(&row)),
                            _ => rows.push(parse_row(&row, &columns)?),
                        }
                    }
                    sets.push(ResultSet { rows });
                }
                None => return Err(Error::Protocol("an empty reply".to_string())),
            }
            if !more {
                return Ok(sets);
            }
        }
    }

    fn write_packet(&mut self, payload: &[u8]) -> Result<(), Error> {
        let mut rest = payload;
        loop {
            let take = rest.len().min(MAX_PACKET);
            let mut frame = Vec::with_capacity(take + 4);
            frame.extend_from_slice(&(take as u32).to_le_bytes()[..3]);
            frame.push(self.seq);
            self.seq = self.seq.wrapping_add(1);
            frame.extend_from_slice(&rest[..take]);
            send_all(&self.stream, &frame)?;
            rest = &rest[take..];
            if take < MAX_PACKET {
                return Ok(());
            }
        }
    }

    fn read_packet(&mut self) -> Result<Vec<u8>, Error> {
        let mut payload = Vec::new();
        loop {
            let mut header = [0u8; 4];
            self.stream.read_exact(&mut header)?;
            let length =
                header[0] as usize | (header[1] as usize) << 8 | (header[2] as usize) << 16;
            self.seq = header[3].wrapping_add(1);
            let start = payload.len();
            payload.resize(start + length, 0);
            self.stream.read_exact(&mut payload[start..])?;
            if length < MAX_PACKET {
                return Ok(payload);
            }
        }
    }
}

/// `tsk` restores the default action of SIGPIPE, so a write to a socket whose server is gone
/// would end the process. These two make the write fail with an error instead.
#[cfg(target_vendor = "apple")]
fn no_sigpipe(stream: &UnixStream) -> Result<(), Error> {
    use std::os::unix::io::AsRawFd;
    let on: libc::c_int = 1;
    let result = unsafe {
        libc::setsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            &on as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(Error::Io(std::io::Error::last_os_error()))
    }
}

#[cfg(not(target_vendor = "apple"))]
fn no_sigpipe(_: &UnixStream) -> Result<(), Error> {
    Ok(())
}

#[cfg(target_os = "linux")]
const SEND_FLAGS: libc::c_int = libc::MSG_NOSIGNAL;
#[cfg(not(target_os = "linux"))]
const SEND_FLAGS: libc::c_int = 0;

/// Write all of `buf`, without raising SIGPIPE when the peer has closed the socket.
fn send_all(stream: &UnixStream, mut buf: &[u8]) -> Result<(), Error> {
    use std::os::unix::io::AsRawFd;
    while !buf.is_empty() {
        let sent = unsafe {
            libc::send(
                stream.as_raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                buf.len(),
                SEND_FLAGS,
            )
        };
        if sent < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(Error::Io(error));
        }
        buf = &buf[sent as usize..];
    }
    Ok(())
}

impl Drop for Conn {
    fn drop(&mut self) {
        // COM_QUIT, best effort. The write timeout bounds it.
        self.seq = 0;
        let _ = self.write_packet(&[0x01]);
    }
}

fn parse_error(packet: &[u8]) -> Error {
    // 0xff, code (2), '#' and a 5 byte state, then the message.
    let code = packet
        .get(1..3)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .unwrap_or(0);
    let body = if packet.get(3) == Some(&b'#') {
        packet.get(9..).unwrap_or(&[])
    } else {
        packet.get(3..).unwrap_or(&[])
    };
    Error::Server {
        code,
        message: String::from_utf8_lossy(body).into_owned(),
    }
}

fn ok_status(packet: &[u8]) -> Result<u16, Error> {
    let (_, at) = read_lenenc(packet, 1)?;
    let (_, at) = read_lenenc(packet, at)?;
    packet
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| Error::Protocol("a short OK packet".to_string()))
}

fn eof_status(packet: &[u8]) -> u16 {
    packet
        .get(3..5)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

/// A length-encoded integer at `at`; returns the value and the position after it.
fn read_lenenc(buf: &[u8], at: usize) -> Result<(u64, usize), Error> {
    let short = || Error::Protocol("a short packet".to_string());
    let first = *buf.get(at).ok_or_else(short)?;
    let width = match first {
        0..=0xfa => return Ok((first as u64, at + 1)),
        0xfc => 2,
        0xfd => 3,
        0xfe => 8,
        _ => return Err(Error::Protocol("a bad length".to_string())),
    };
    let bytes = buf.get(at + 1..at + 1 + width).ok_or_else(short)?;
    let mut value = 0u64;
    for (i, b) in bytes.iter().enumerate() {
        value |= (*b as u64) << (8 * i);
    }
    Ok((value, at + 1 + width))
}

/// A length-encoded string at `at`: `None` for NULL.
fn read_lenenc_str(buf: &[u8], at: usize) -> Result<(Option<&[u8]>, usize), Error> {
    if buf.get(at) == Some(&0xfb) {
        return Ok((None, at + 1));
    }
    let (len, at) = read_lenenc(buf, at)?;
    let end = at + len as usize;
    let bytes = buf
        .get(at..end)
        .ok_or_else(|| Error::Protocol("a short string".to_string()))?;
    Ok((Some(bytes), end))
}

fn parse_column(def: &[u8]) -> Result<Column, Error> {
    let mut at = 0;
    let mut name = String::new();
    // catalog, schema, table, original table, name, original name
    for index in 0..6 {
        let (text, next) = read_lenenc_str(def, at)?;
        if index == 4 {
            name = String::from_utf8_lossy(text.unwrap_or(&[])).into_owned();
        }
        at = next;
    }
    // The length of the fixed fields (0x0c), the character set (2), the column length (4),
    // the type (1), the flags (2) and the decimals (1).
    let (_, at) = read_lenenc(def, at)?;
    let fixed = def
        .get(at..at + 10)
        .ok_or_else(|| Error::Protocol("a short column definition".to_string()))?;
    Ok(Column {
        name,
        kind: fixed[6],
        unsigned: u16::from_le_bytes([fixed[7], fixed[8]]) & UNSIGNED_FLAG != 0,
    })
}

fn parse_row(row: &[u8], columns: &[Column]) -> Result<Value, Error> {
    let mut at = 0;
    let mut map = Map::new();
    for column in columns {
        let (bytes, next) = read_lenenc_str(row, at)?;
        at = next;
        // `dolt sql -r json` leaves a NULL column out of the row.
        if let Some(bytes) = bytes {
            map.insert(column.name.clone(), to_json(column, bytes));
        }
    }
    Ok(Value::Object(map))
}

/// A value as `dolt sql -r json` prints it: integers and floats are numbers, a JSON column
/// is its parsed value, and every other type, decimals included, is a string.
fn to_json(column: &Column, bytes: &[u8]) -> Value {
    let text = String::from_utf8_lossy(bytes);
    match column.kind {
        TYPE_TINY | TYPE_SHORT | TYPE_LONG | TYPE_LONGLONG | TYPE_INT24 | TYPE_YEAR => {
            let parsed = if column.unsigned {
                text.parse::<u64>().ok().map(Number::from)
            } else {
                text.parse::<i64>().ok().map(Number::from)
            };
            parsed
                .or_else(|| text.parse::<u64>().ok().map(Number::from))
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(text.into_owned()))
        }
        TYPE_FLOAT | TYPE_DOUBLE => text
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(text.into_owned())),
        TYPE_JSON => {
            serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.into_owned()))
        }
        TYPE_DECIMAL | TYPE_NEWDECIMAL => Value::String(text.into_owned()),
        _ => Value::String(text.into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packet with the sequence number `seq`.
    fn frame(seq: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32).to_le_bytes()[..3].to_vec();
        out.push(seq);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn a_server_that_goes_silent_ends_a_query_with_a_timeout() {
        use std::io::{Read, Write};
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("fake.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // A greeting, the login, an OK, then silence for the query.
            stream.write_all(&frame(0, &[10, b'x', 0])).unwrap();
            let mut header = [0u8; 4];
            stream.read_exact(&mut header).unwrap();
            let length = header[0] as usize | (header[1] as usize) << 8;
            let mut login = vec![0u8; length];
            stream.read_exact(&mut login).unwrap();
            stream.write_all(&frame(2, &[0, 0, 0, 2, 0, 0, 0])).unwrap();
            std::thread::sleep(Duration::from_millis(1500));
        });
        let mut conn = Conn::connect(&socket, "db", Some(Duration::from_millis(500))).unwrap();
        let started = std::time::Instant::now();
        let error = conn.query("select 1").unwrap_err();
        assert!(error.is_connection(), "{}", error);
        assert!(started.elapsed() < Duration::from_millis(1400));
        drop(conn);
        server.join().unwrap();
    }

    #[test]
    fn a_write_to_a_closed_socket_is_an_error() {
        // The test runtime ignores SIGPIPE. `tsk` restores its default action, and the send
        // flag is what keeps a write here from ending the process.
        let (ours, theirs) = UnixStream::pair().unwrap();
        drop(theirs);
        assert!(matches!(send_all(&ours, b"hello"), Err(Error::Io(_))));
    }

    #[test]
    fn length_encoded_integers_read_every_width() {
        assert_eq!(read_lenenc(&[5], 0).unwrap(), (5, 1));
        assert_eq!(read_lenenc(&[0xfc, 0x01, 0x01], 0).unwrap(), (257, 3));
        assert_eq!(read_lenenc(&[0xfd, 1, 0, 1], 0).unwrap(), (65537, 4));
        assert_eq!(
            read_lenenc(&[0xfe, 1, 0, 0, 0, 0, 0, 0, 0], 0).unwrap(),
            (1, 9)
        );
        assert!(read_lenenc(&[0xfc, 1], 0).is_err());
    }

    #[test]
    fn a_row_leaves_out_null_and_converts_by_type() {
        let columns = vec![
            Column {
                name: "a".into(),
                kind: TYPE_LONGLONG,
                unsigned: false,
            },
            Column {
                name: "b".into(),
                kind: 0xfd,
                unsigned: false,
            },
            Column {
                name: "c".into(),
                kind: TYPE_NEWDECIMAL,
                unsigned: false,
            },
            Column {
                name: "d".into(),
                kind: 0xfd,
                unsigned: false,
            },
        ];
        let row = [
            &[1u8, b'7'][..],
            &[2, b'h', b'i'],
            &[3, b'1', b'.', b'5'],
            &[0xfb],
        ]
        .concat();
        let value = parse_row(&row, &columns).unwrap();
        assert_eq!(value["a"], 7);
        assert_eq!(value["b"], "hi");
        assert_eq!(value["c"], "1.5");
        assert!(value.get("d").is_none());
    }

    #[test]
    fn an_error_packet_carries_the_code_and_the_message() {
        let mut packet = vec![0xff, 0x7a, 0x04];
        packet.extend_from_slice(b"#42S02table not found: bindings");
        match parse_error(&packet) {
            Error::Server { code, message } => {
                assert_eq!(code, 1146);
                assert_eq!(message, "table not found: bindings");
            }
            other => panic!("{:?}", other),
        }
    }
}
