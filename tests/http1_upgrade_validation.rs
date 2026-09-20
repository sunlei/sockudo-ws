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
fn request_accepts_ows_around_required_header_values() {
    let request = b"GET /ws HTTP/1.1\r\n\
        Host:\t example.com \t\r\n\
        Upgrade: WebSocket\r\n\
        Connection: keep-alive, UpGrAdE\r\n\
        Sec-WebSocket-Key:\t dGhlIHNhbXBsZSBub25jZQ== \t\r\n\
        Sec-WebSocket-Version:\t 13 \t\r\n\
        \r\n";

    let (request, _) = parse_request(request).unwrap().unwrap();

    assert_eq!(request.host, Some("example.com"));
    assert_eq!(request.key, KEY);
    assert_eq!(request.version, "13");
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

#[test]
fn response_accepts_case_insensitive_tokens_in_lists() {
    let response = b"HTTP/1.1 101 Switching Protocols\r\n\
        Upgrade:\t h2c, WebSocket \t\r\n\
        Connection: keep-alive, UpGrAdE\r\n\
        \r\n";

    assert!(parse_response(response).unwrap().is_some());
}
