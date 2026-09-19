use sockudo_ws::Error;
use sockudo_ws::handshake::{parse_request, parse_response};

const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

fn request(version: &str, host: Option<&str>, key: &str) -> Vec<u8> {
    let host = host
        .map(|value| format!("Host: {value}\r\n"))
        .unwrap_or_default();
    format!(
        "GET /ws HTTP/{version}\r\n\
         {host}\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    )
    .into_bytes()
}

#[test]
fn request_requires_http_11() {
    let error = parse_request(&request("1.0", Some("example.com"), KEY)).unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidHttp("HTTP version must be 1.1")
    ));
}

#[test]
fn request_requires_host() {
    let error = parse_request(&request("1.1", None, KEY)).unwrap_err();

    assert!(matches!(error, Error::HandshakeFailed("missing Host")));
}

#[test]
fn request_requires_a_base64_encoded_16_byte_key() {
    for key in ["not base64", "YWJjZA==", "YWJjZGVmZ2hpamtsbW5vcHE="] {
        let error = parse_request(&request("1.1", Some("example.com"), key)).unwrap_err();

        assert!(matches!(
            error,
            Error::HandshakeFailed("invalid Sec-WebSocket-Key")
        ));
    }
}

#[test]
fn response_requires_http_11() {
    let response = b"HTTP/1.0 101 Switching Protocols\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        \r\n";

    let error = parse_response(response).unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidHttp("HTTP version must be 1.1")
    ));
}

#[test]
fn response_requires_upgrade_and_connection_tokens() {
    for response in [
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\n\r\n".as_slice(),
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n".as_slice(),
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: notwebsocket\r\nConnection: Upgrade\r\n\r\n"
            .as_slice(),
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: keep-alive\r\n\r\n"
            .as_slice(),
    ] {
        assert!(parse_response(response).is_err());
    }
}
