#![cfg(all(feature = "compio-runtime", feature = "http3"))]

use std::sync::Once;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use sockudo_ws::compio::{CompioHttp3Server, connect_http3};
use sockudo_ws::{Config, Message};

static INSTALL_CRYPTO: Once = Once::new();

fn tls_configs() -> (rustls::ServerConfig, rustls::ClientConfig) {
    INSTALL_CRYPTO.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(key_pair.serialize_der()).unwrap();

    let server = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .unwrap();

    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert_der).unwrap();
    let client = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    (server, client)
}

#[compio::test]
async fn cancelled_client_write_resumes_pending_http3_data() {
    const PAYLOAD_SIZE: usize = 4 * 1024 * 1024;

    let (server_tls, client_tls) = tls_configs();
    let endpoint = compio::quic::ServerBuilder::new_with_rustls_server_config(server_tls)
        .with_alpn_protocols(&["h3"])
        .bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr = endpoint.local_addr().unwrap();
    let server = CompioHttp3Server::from_endpoint(endpoint.clone(), Config::default());
    let (received_tx, mut received_rx) = futures_channel::mpsc::unbounded();

    let server_task = compio::runtime::spawn(async move {
        server
            .serve(move |mut ws, _| {
                let mut received_tx = received_tx.clone();
                async move {
                    compio::time::sleep(Duration::from_millis(100)).await;
                    let first = ws.next().await.unwrap().unwrap();
                    assert!(matches!(first, Message::Binary(data) if data.len() == PAYLOAD_SIZE));
                    let second = ws.next().await.unwrap().unwrap();
                    assert!(matches!(second, Message::Text(text) if text == "after-cancel"));
                    received_tx.send(()).await.unwrap();
                }
            })
            .await
            .unwrap();
    });

    let mut client = connect_http3(
        addr,
        "localhost",
        "/cancelled-write",
        None,
        client_tls,
        Config::builder()
            .max_backpressure(PAYLOAD_SIZE + 14)
            .build(),
    )
    .await
    .unwrap();

    let cancelled = compio::time::timeout(
        Duration::from_millis(10),
        client.send(Message::binary(vec![0x5a; PAYLOAD_SIZE])),
    )
    .await;
    assert!(
        cancelled.is_err(),
        "large write completed before cancellation"
    );

    compio::time::timeout(Duration::from_secs(5), client.send_text("after-cancel"))
        .await
        .expect("replacement write timed out")
        .expect("replacement write failed");
    compio::time::timeout(Duration::from_secs(5), received_rx.next())
        .await
        .expect("server receive timed out")
        .expect("server stopped before receiving both messages");

    endpoint.close(compio::quic::VarInt::from_u32(0x100), b"done");
    server_task.await.unwrap();
}

#[compio::test]
async fn cancelled_server_write_resumes_pending_http3_data() {
    const PAYLOAD_SIZE: usize = 4 * 1024 * 1024;

    let (server_tls, client_tls) = tls_configs();
    let endpoint = compio::quic::ServerBuilder::new_with_rustls_server_config(server_tls)
        .with_alpn_protocols(&["h3"])
        .bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr = endpoint.local_addr().unwrap();
    let server = CompioHttp3Server::from_endpoint(
        endpoint.clone(),
        Config::builder()
            .max_backpressure(PAYLOAD_SIZE + 14)
            .build(),
    );
    let (sent_tx, mut sent_rx) = futures_channel::mpsc::unbounded();

    let server_task = compio::runtime::spawn(async move {
        server
            .serve(move |mut ws, _| {
                let mut sent_tx = sent_tx.clone();
                async move {
                    let cancelled = compio::time::timeout(
                        Duration::from_millis(10),
                        ws.send(Message::binary(vec![0xa5; PAYLOAD_SIZE])),
                    )
                    .await;
                    assert!(
                        cancelled.is_err(),
                        "large write completed before cancellation"
                    );
                    let result =
                        compio::time::timeout(Duration::from_secs(5), ws.send_text("after-cancel"))
                            .await
                            .map_err(|_| "replacement write timed out".to_string())
                            .and_then(|result| result.map_err(|error| error.to_string()));
                    sent_tx.send(result).await.unwrap();
                }
            })
            .await
            .unwrap();
    });

    let mut client = connect_http3(
        addr,
        "localhost",
        "/cancelled-server-write",
        None,
        client_tls,
        Config::default(),
    )
    .await
    .unwrap();
    compio::time::sleep(Duration::from_millis(100)).await;

    let first = compio::time::timeout(Duration::from_secs(5), client.next())
        .await
        .expect("first message timed out")
        .expect("server stopped before the first message")
        .unwrap();
    assert!(matches!(first, Message::Binary(data) if data.len() == PAYLOAD_SIZE));
    let second = compio::time::timeout(Duration::from_secs(5), client.next())
        .await
        .expect("second message timed out")
        .expect("server stopped before the second message")
        .unwrap();
    assert!(matches!(second, Message::Text(text) if text == "after-cancel"));
    sent_rx
        .next()
        .await
        .expect("server stopped before reporting the replacement write")
        .expect("replacement write failed");

    endpoint.close(compio::quic::VarInt::from_u32(0x100), b"done");
    server_task.await.unwrap();
}
