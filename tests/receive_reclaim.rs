#![cfg(all(feature = "tokio-runtime", feature = "permessage-deflate"))]

#[path = "support/reclaim.rs"]
mod support;
use rstest::rstest;
use std::sync::Arc;
use support::{Input, Reader, pattern, wire};

#[rstest]
#[case::plain(false, false)]
#[case::split(false, true)]
#[case::compressed(true, false)]
#[case::compressed_split(true, true)]
#[tokio::test(flavor = "current_thread")]
async fn retained_messages_survive_receive_window_reuse(
    #[case] compressed: bool,
    #[case] split: bool,
    #[values(false, true)] large: bool,
) {
    let payload = if large {
        // Keep the wire input above half a receive window even with compression.
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut bytes: Vec<u8> = (0..48 * 1024)
            .map(|_| {
                state ^= state >> 12;
                state ^= state << 25;
                state ^= state >> 27;
                (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
            })
            .collect();
        bytes.resize(64 * 1024, b'Z');
        bytes.into()
    } else {
        pattern(4096)
    };
    let alternate: Vec<u8> = payload.iter().map(|byte| byte ^ 0x55).collect();
    let first_frame = wire(&payload, compressed, 1);
    let second_frame = wire(&alternate, compressed, 1);
    if large {
        assert!(first_frame.len() > sockudo_ws::RECV_BUFFER_SIZE / 2);
        assert!(second_frame.len() > sockudo_ws::RECV_BUFFER_SIZE / 2);
    }
    if compressed {
        assert_ne!(first_frame[0] & 0x40, 0);
        assert_ne!(second_frame[0] & 0x40, 0);
    }
    let mut frames = first_frame.as_ref().clone();
    frames.extend_from_slice(&second_frame);
    let input = Input {
        wire: Arc::new(frames.repeat(4)),
        offset: 0,
        pending: false,
        inject_pending: true,
        max_read: 1301,
    };
    let mut reader = Reader::new(input, compressed, split);
    // Hold a receive allocation across several full windows, then release it.
    let first = reader.next().await;
    for index in 0..256 {
        assert_eq!(
            reader.next().await.as_bytes(),
            if index % 2 == 0 {
                alternate.as_slice()
            } else {
                payload.as_ref()
            }
        );
    }
    assert_eq!(first.as_bytes(), payload.as_ref());
    drop(first);
    for index in 0..256 {
        assert_eq!(
            reader.next().await.as_bytes(),
            if index % 2 == 0 {
                alternate.as_slice()
            } else {
                payload.as_ref()
            }
        );
    }
}
