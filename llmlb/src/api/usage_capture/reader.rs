//! Bounded asynchronous-body bridge for serde's streaming metadata reader.

use super::{metadata, UsageCollector};
use axum::body::Bytes;
use std::future::Future;
use std::io::{self, Read};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio_util::sync::PollSender;

const CHUNK_BYTES: usize = 8 * 1024;
const QUEUED_CHUNKS: usize = 2;
const PARSER_SLOTS: usize = 64;
static PARSERS: OnceLock<Arc<Semaphore>> = OnceLock::new();

struct ChunkReader {
    receiver: mpsc::Receiver<Bytes>,
    current: Bytes,
    offset: usize,
}

impl Read for ChunkReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.offset == self.current.len() {
            let Some(next) = self.receiver.blocking_recv() else {
                return Ok(0);
            };
            self.current = next;
            self.offset = 0;
        }
        let len = out.len().min(self.current.len() - self.offset);
        out[..len].copy_from_slice(&self.current[self.offset..self.offset + len]);
        self.offset += len;
        Ok(len)
    }
}

struct FinalizeOnDrop(Option<UsageCollector>);
impl Drop for FinalizeOnDrop {
    fn drop(&mut self) {
        if let Some(collector) = self.0.take() {
            collector.finish(Default::default(), false);
        }
    }
}

/// Holds at most two fixed-size copies, and waits on channel progress rather
/// than scheduling a busy polling loop when the parser is backpressured.
pub(super) struct MetadataReader {
    sender: PollSender<Bytes>,
    completed: oneshot::Receiver<()>,
    transport_complete: Arc<AtomicBool>,
    closed: bool,
    _worker: tokio::task::JoinHandle<()>,
}

impl MetadataReader {
    pub fn new(collector: UsageCollector, successful: bool, sse: bool) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUED_CHUNKS);
        let (completed_sender, completed) = oneshot::channel();
        let transport_complete = Arc::new(AtomicBool::new(false));
        let complete = transport_complete.clone();
        let registration = collector.storage.track_usage_task();
        let slots = PARSERS
            .get_or_init(|| Arc::new(Semaphore::new(PARSER_SLOTS)))
            .clone();
        let worker = tokio::spawn(async move {
            // Declaration order ensures the fallback enqueues a terminal write
            // before cancellation/panic releases the shutdown registration.
            let _registration = registration;
            let mut fallback = FinalizeOnDrop(Some(collector.clone()));
            let Ok(_permit) = slots.acquire_owned().await else {
                return;
            };
            let result = tokio::task::spawn_blocking(move || {
                let reader = io::BufReader::new(ChunkReader {
                    receiver,
                    current: Bytes::new(),
                    offset: 0,
                });
                if sse {
                    metadata::read_sse(reader)
                } else {
                    metadata::read_json(reader)
                }
            })
            .await;
            let metadata = match result {
                Ok(metadata) => metadata,
                Err(error) => {
                    tracing::error!(%error, "Usage metadata reader failed");
                    metadata::Metadata {
                        failed: true,
                        ..Default::default()
                    }
                }
            };
            collector.finish(
                metadata.tokens,
                complete.load(Ordering::SeqCst) && successful && !metadata.failed,
            );
            fallback.0 = None;
            let _ = completed_sender.send(());
        });
        Self {
            sender: PollSender::new(sender),
            completed,
            transport_complete,
            closed: false,
            _worker: worker,
        }
    }

    pub fn poll_send(
        &mut self,
        bytes: &[u8],
        offset: &mut usize,
        cx: &mut Context<'_>,
    ) -> Poll<()> {
        while *offset < bytes.len() {
            match self.sender.poll_reserve(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(_)) => return Poll::Ready(()),
                Poll::Ready(Ok(())) => {
                    let end = bytes.len().min(*offset + CHUNK_BYTES);
                    if self
                        .sender
                        .send_item(Bytes::copy_from_slice(&bytes[*offset..end]))
                        .is_err()
                    {
                        return Poll::Ready(());
                    }
                    *offset = end;
                }
            }
        }
        Poll::Ready(())
    }

    pub fn close(&mut self, complete: bool) {
        if !self.closed {
            self.transport_complete.store(complete, Ordering::SeqCst);
            self.closed = true;
            self.sender.close();
        }
    }

    pub fn poll_completed(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.completed).poll(cx).map(|_| ())
    }
}

impl Drop for MetadataReader {
    fn drop(&mut self) {
        self.close(false);
    }
}
