//! Integration tests for StreamManager audio pipeline using mock providers.
//! Fully deterministic — no Azure API, no audio hardware, no browser.
//!
//! Run: cargo test --test stream_manager_test

use async_trait::async_trait;
use beme_lib::ai::{AiError, AudioResponseRx, AudioSession};
use beme_lib::stream_manager::StreamManager;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Mock implementations
// ---------------------------------------------------------------------------

struct MockAudioSession {
    sender: mpsc::UnboundedSender<Vec<u8>>,
    closed: bool,
}

#[async_trait]
impl AudioSession for MockAudioSession {
    async fn send_audio(&mut self, audio_data: &[u8]) -> Result<(), AiError> {
        if self.closed {
            return Err(AiError::ConnectionError("Session closed".into()));
        }
        self.sender
            .send(audio_data.to_vec())
            .map_err(|e| AiError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), AiError> {
        self.closed = true;
        Ok(())
    }
}

fn mock_session() -> (MockAudioSession, mpsc::UnboundedReceiver<Vec<u8>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        MockAudioSession {
            sender: tx,
            closed: false,
        },
        rx,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Audio chunks sent via `process_audio_chunk` reach the injected mock session.
#[tokio::test]
async fn audio_chunks_reach_session() {
    let sm = StreamManager::new();
    let (session, mut rx) = mock_session();
    sm.inject_audio_session(Box::new(session)).await;

    let chunk = vec![1u8, 2, 3, 4, 5];
    sm.process_audio_chunk(&chunk)
        .await
        .expect("should succeed");

    let received = rx.recv().await.expect("should receive data");
    assert_eq!(received, chunk);
}

/// Multiple chunks arrive in the order they were sent.
#[tokio::test]
async fn multiple_chunks_arrive_in_order() {
    let sm = StreamManager::new();
    let (session, mut rx) = mock_session();
    sm.inject_audio_session(Box::new(session)).await;

    let chunks: Vec<Vec<u8>> = vec![vec![10, 20], vec![30, 40, 50], vec![60]];
    for c in &chunks {
        sm.process_audio_chunk(c).await.unwrap();
    }
    for expected in &chunks {
        assert_eq!(&rx.recv().await.unwrap(), expected);
    }
}

/// `process_audio_chunk` returns an error when no session is active.
#[tokio::test]
async fn process_audio_chunk_fails_without_session() {
    let sm = StreamManager::new();
    let err = sm.process_audio_chunk(&[1, 2, 3]).await.unwrap_err();
    assert_eq!(err, "No active audio session");
}

/// Session lifecycle: inject → active → clear → inactive.
#[tokio::test]
async fn session_lifecycle() {
    let sm = StreamManager::new();

    // Initially no session
    assert!(!sm.has_audio_session().await);

    // Inject session → active
    let (session, _rx) = mock_session();
    sm.inject_audio_session(Box::new(session)).await;
    assert!(sm.has_audio_session().await);

    // process_audio_chunk works
    sm.process_audio_chunk(&[1]).await.unwrap();

    // Clear session → inactive
    sm.clear_audio_session().await;
    assert!(!sm.has_audio_session().await);

    // process_audio_chunk fails
    assert!(sm.process_audio_chunk(&[1]).await.is_err());
}

/// The `AudioResponseRx` channel correctly delivers canned text responses,
/// matching the pattern used by the reader task in `start_audio_session`.
#[tokio::test]
async fn response_channel_delivers_canned_responses() {
    let (tx, mut rx): (mpsc::Sender<Result<String, AiError>>, AudioResponseRx) = mpsc::channel(16);

    tx.send(Ok("Hello".into())).await.unwrap();
    tx.send(Ok(" world".into())).await.unwrap();
    tx.send(Ok(String::new())).await.unwrap(); // turn-done signal
    drop(tx);

    let mut texts = Vec::new();
    while let Some(result) = rx.recv().await {
        texts.push(result.unwrap());
    }
    assert_eq!(texts, vec!["Hello", " world", ""]);
}

/// Errors on the response channel are propagated correctly.
#[tokio::test]
async fn response_channel_propagates_errors() {
    let (tx, mut rx): (mpsc::Sender<Result<String, AiError>>, AudioResponseRx) = mpsc::channel(16);

    tx.send(Ok("partial".into())).await.unwrap();
    tx.send(Err(AiError::ConnectionError("lost connection".into())))
        .await
        .unwrap();
    drop(tx);

    let first = rx.recv().await.unwrap().unwrap();
    assert_eq!(first, "partial");

    let second = rx.recv().await.unwrap();
    assert!(second.is_err());
    assert!(second.unwrap_err().to_string().contains("lost connection"));
}

/// A closed `MockAudioSession` rejects further `send_audio` calls.
#[tokio::test]
async fn closed_session_rejects_audio() {
    let (mut session, _rx) = mock_session();

    session.send_audio(&[1, 2]).await.unwrap();
    session.close().await.unwrap();

    let result = session.send_audio(&[3, 4]).await;
    assert!(result.is_err());
}
