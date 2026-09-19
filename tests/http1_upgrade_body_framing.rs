use sockudo_ws::Error;
use sockudo_ws::handshake::{build_request_with_headers, parse_request};

const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

fn request_with(header: &str) -> Vec<u8> {
    format!(
        "GET /ws HTTP/1.1\r\n\
         Host: example.com\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {KEY}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         {header}\
         \r\n"
    )
    .into_bytes()
}

#[test]
fn request_accepts_zero_content_length_values() {
    for header in [
        "Content-Length: 0\r\n",
        "Content-Length: 00\r\n",
        "Content-Length: 0, 0\r\n",
        "Content-Length: 0\r\nContent-Length: 0\r\n",
    ] {
        assert!(parse_request(&request_with(header)).unwrap().is_some());
    }
}

#[test]
fn request_rejects_nonzero_or_conflicting_content_length() {
    for header in [
        "Content-Length: 1\r\n",
        "Content-Length: 0, 1\r\n",
        "Content-Length: 0\r\nContent-Length: 1\r\n",
    ] {
        let error = parse_request(&request_with(header)).unwrap_err();

        assert!(matches!(
            error,
            Error::InvalidHttp("WebSocket handshake must not contain a body")
        ));
    }
}

#[test]
fn request_rejects_transfer_encoding() {
    let error = parse_request(&request_with("Transfer-Encoding: chunked\r\n")).unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidHttp("WebSocket handshake must not use Transfer-Encoding")
    ));
}

#[test]
fn request_builder_rejects_body_framing_headers() {
    for name in ["Content-Length", "Transfer-Encoding"] {
        let headers = [(name.to_string(), "0".to_string())];
        let error =
            build_request_with_headers("example.com", "/ws", KEY, None, None, Some(&headers))
                .unwrap_err();

        assert!(matches!(
            error,
            Error::InvalidHttp("reserved handshake header")
        ));
    }
}
