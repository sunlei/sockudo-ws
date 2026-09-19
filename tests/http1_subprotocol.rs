mod support;

use sockudo_ws::Error;
use sockudo_ws::handshake::{server_handshake, server_handshake_with_protocols};
use support::{extract_header, read_http_request};
use tokio::io::AsyncWriteExt;

const REQUEST: &[u8] = b"GET /ws HTTP/1.1\r\n\
    Host: example.com\r\n\
    Upgrade: websocket\r\n\
    Connection: Upgrade\r\n\
    Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
    Sec-WebSocket-Version: 13\r\n\
    Sec-WebSocket-Protocol: chat, superchat\r\n\
    \r\n";

#[tokio::test]
async fn default_server_does_not_echo_offered_subprotocols() {
    let (mut client_io, mut server_io) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move { server_handshake(&mut server_io).await.unwrap() });

    client_io.write_all(REQUEST).await.unwrap();
    let response = read_http_request(&mut client_io).await;
    let response = String::from_utf8(response).unwrap();
    let handshake = server.await.unwrap();

    assert_eq!(extract_header(&response, "Sec-WebSocket-Protocol"), None);
    assert_eq!(handshake.protocol, None);
}

#[tokio::test]
async fn server_selects_an_offered_subprotocol_in_server_preference_order() {
    let (mut client_io, mut server_io) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move {
        server_handshake_with_protocols(&mut server_io, ["superchat", "chat"])
            .await
            .unwrap()
    });

    client_io.write_all(REQUEST).await.unwrap();
    let response = read_http_request(&mut client_io).await;
    let response = String::from_utf8(response).unwrap();
    let handshake = server.await.unwrap();

    assert_eq!(
        extract_header(&response, "Sec-WebSocket-Protocol"),
        Some("superchat")
    );
    assert_eq!(handshake.protocol.as_deref(), Some("superchat"));
}

#[tokio::test]
async fn server_rejects_invalid_supported_subprotocols_before_reading() {
    let (_client_io, mut server_io) = tokio::io::duplex(4096);

    let error = server_handshake_with_protocols(&mut server_io, ["invalid protocol"])
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::InvalidHttp("invalid supported subprotocol")
    ));
}
