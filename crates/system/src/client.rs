use std::os::unix::net::UnixStream;

use anyhow::{Result, anyhow};

use crate::protocol::{self, Request, Response, SOCKET_PATH};

// Taken by value because the request is serialised straight onto the socket and
// there is no caller left to reuse it; borrowing would buy nothing, since
// encoding is the only use the value gets.
#[allow(clippy::needless_pass_by_value)]
/// Sends a request to the running daemon and waits for its reply.
///
/// # Errors
///
/// Returns an error if the daemon socket cannot be reached, or if the daemon
/// replies with something this version cannot decode.
pub fn send_request(request: Request) -> Result<Response> {
    let mut stream = UnixStream::connect(SOCKET_PATH).map_err(|e| {
        anyhow!("Failed to connect to zoid daemon at {SOCKET_PATH}: {e}")
    })?;

    protocol::send_message(&mut stream, &request)?;
    let response: Response = protocol::receive_message(&mut stream)?;
    Ok(response)
}
