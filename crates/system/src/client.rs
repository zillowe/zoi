use crate::protocol::{self, Request, Response};
use anyhow::{Result, anyhow};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/run/zoid.sock";

pub fn send_request(request: Request) -> Result<Response> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| anyhow!("Failed to connect to zoid daemon at {}: {}", SOCKET_PATH, e))?;

    protocol::send_message(&mut stream, &request)?;
    let response: Response = protocol::receive_message(&mut stream)?;
    Ok(response)
}
