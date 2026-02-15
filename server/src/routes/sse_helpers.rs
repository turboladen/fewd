use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use serde::Serialize;
use tokio::sync::mpsc;

use crate::services::claude_client::ProgressEvent;

/// Payload types that flow through the SSE channel from the spawned task to the response stream
#[derive(Debug)]
pub enum SsePayload {
    /// Forwarded from ClaudeClient streaming progress
    Progress(ProgressEvent),
    /// Final parsed result (serialized as JSON value)
    Complete(serde_json::Value),
    /// Error during generation or parsing
    Error(String),
}

/// Convert an mpsc receiver of `SsePayload` into an Axum SSE response.
/// The stream yields `event: progress`, `event: complete`, or `event: error` SSE events.
pub fn sse_from_channel(
    mut rx: mpsc::Receiver<SsePayload>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        while let Some(payload) = rx.recv().await {
            match payload {
                SsePayload::Progress(event) => {
                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok::<_, Infallible>(Event::default().event("progress").data(data));
                    }
                }
                SsePayload::Complete(value) => {
                    #[derive(Serialize)]
                    struct CompleteEvent {
                        phase: &'static str,
                        data: serde_json::Value,
                    }
                    let event = CompleteEvent {
                        phase: "complete",
                        data: value,
                    };
                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok::<_, Infallible>(Event::default().event("complete").data(data));
                    }
                    break; // Stream ends after complete
                }
                SsePayload::Error(message) => {
                    #[derive(Serialize)]
                    struct ErrorEvent<'a> {
                        phase: &'static str,
                        message: &'a str,
                    }
                    let event = ErrorEvent {
                        phase: "error",
                        message: &message,
                    };
                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok::<_, Infallible>(Event::default().event("error").data(data));
                    }
                    break; // Stream ends after error
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// Helper to forward ProgressEvents from the Claude client channel to the SSE payload channel
pub async fn forward_progress(
    mut progress_rx: mpsc::Receiver<ProgressEvent>,
    sse_tx: mpsc::Sender<SsePayload>,
) {
    while let Some(event) = progress_rx.recv().await {
        if sse_tx.send(SsePayload::Progress(event)).await.is_err() {
            break; // Frontend disconnected
        }
    }
}
