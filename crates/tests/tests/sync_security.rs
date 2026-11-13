//! Security regression tests for registry synchronization.
//!
//! Registry handles come from remote `repo.yaml` files and project
//! configuration and are used as path components, so they must be validated.

use std::io::{Read, Write};
use std::net::TcpListener;

use zoi_core::utils::validate_registry_handle;

mod common;

#[test]
fn registry_handles_are_validated() {
    for ok in ["zoidberg", "my-registry", "reg_1.0", "A.b-C_d"] {
        assert!(validate_registry_handle(ok).is_ok(), "{ok} should pass");
    }
    let long = "a".repeat(200);
    for bad in [
        "",
        ".",
        "..",
        "../evil",
        "a/b",
        "a\\b",
        "/abs",
        "with space",
        "semi;colon",
        "nul\0",
        long.as_str()
    ] {
        assert!(
            validate_registry_handle(bad).is_err(),
            "{bad:?} should be rejected"
        );
    }
}

#[test]
fn registry_with_traversal_name_is_rejected_on_add() {
    let mut ctx = common::TestContextGuard::acquire();
    ctx.set_env_var("NO_PROXY", "127.0.0.1,localhost");
    ctx.set_env_var("no_proxy", "127.0.0.1,localhost");
    common::TestContextGuard::set_offline(false);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = "name: ../../evil\ndescription: x\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: \
                 text/yaml\r\nContent-Length: {}\r\nConnection: \
                 close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    let result = zoi_sync::resolve_registry_from_url(&format!(
        "http://127.0.0.1:{port}/repo.yaml"
    ));
    let _ = server.join();

    let err = result.expect_err("a traversal handle must be rejected");
    assert!(
        err.to_string().contains("Invalid registry handle"),
        "unexpected error: {err}"
    );
}
