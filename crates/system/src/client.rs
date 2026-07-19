use crate::protocol::{Request, Response};
use anyhow::{Result, anyhow};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/run/zoid.sock";

pub fn send_request(request: Request) -> Result<Response> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| anyhow!("Failed to connect to zoid daemon at {}: {}", SOCKET_PATH, e))?;

    let request_bytes = serde_json::to_vec(&request)?;
    stream.write_all(&request_bytes)?;

    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if n < 1024 {
            break;
        }
    }

    let response: Response = serde_json::from_slice(&buffer)?;
    Ok(response)
}
