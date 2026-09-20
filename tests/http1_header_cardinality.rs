use sockudo_ws::Error;
use sockudo_ws::handshake::{parse_request, parse_response};

const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

fn request_with(extra_headers: &str) -> Vec<u8> {
    format!(
        "GET /ws HTTP/1.1\r\n\
         Host: example.com\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {KEY}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         {extra_headers}\
         \r\n"
    )
    .into_bytes()
}

#[test]
fn request_rejects_repeated_key_and_version_fields() {
    for (header, message) in [
        (
            format!("Sec-WebSocket-Key: {KEY}\r\n"),
            "duplicate Sec-WebSocket-Key",
        ),
        (
            "Sec-WebSocket-Version: 13\r\n".to_string(),
            "duplicate Sec-WebSocket-Version",
        ),
    ] {
        let error = parse_request(&request_with(&header)).unwrap_err();

        assert!(matches!(error, Error::HandshakeFailed(actual) if actual == message));
    }
}

#[test]
fn response_rejects_repeated_singleton_websocket_fields() {
    for (header, message) in [
        (
            "Sec-WebSocket-Accept: first\r\nSec-WebSocket-Accept: second\r\n",
            "duplicate Sec-WebSocket-Accept",
        ),
        (
            "Sec-WebSocket-Protocol: chat\r\nSec-WebSocket-Protocol: superchat\r\n",
            "duplicate Sec-WebSocket-Protocol",
        ),
        (
            "Sec-WebSocket-Extensions: one\r\nSec-WebSocket-Extensions: two\r\n",
            "duplicate Sec-WebSocket-Extensions",
        ),
    ] {
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             {header}\
             \r\n"
        );
        let error = parse_response(response.as_bytes()).unwrap_err();

        assert!(matches!(error, Error::HandshakeFailed(actual) if actual == message));
    }
}
