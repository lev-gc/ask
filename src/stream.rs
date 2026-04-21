use bytes::Bytes;
use futures_util::stream::StreamExt;
use futures_util::Stream;

/// Parse an SSE byte-stream into a stream of (event_name, data_payload) pairs.
/// Lines are accumulated; `event: X` and `data: Y` fields are collected until a blank line flushes the record.
pub fn sse_events<S>(byte_stream: S) -> impl Stream<Item = anyhow::Result<SseEvent>>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    async_stream::stream! {
        let mut buf = Vec::<u8>::new();
        let mut event_name: Option<String> = None;
        let mut data_lines: Vec<String> = Vec::new();
        let mut s = byte_stream;
        while let Some(chunk) = s.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => { yield Err(anyhow::anyhow!(e)); return; }
            };
            buf.extend_from_slice(&chunk);
            loop {
                let Some(pos) = buf.iter().position(|b| *b == b'\n') else { break; };
                let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len().saturating_sub(1)]);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    if !data_lines.is_empty() || event_name.is_some() {
                        yield Ok(SseEvent {
                            event: event_name.take().unwrap_or_default(),
                            data: data_lines.join("\n"),
                        });
                        data_lines.clear();
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("data:") {
                    data_lines.push(rest.trim_start().to_string());
                } else if let Some(rest) = line.strip_prefix("event:") {
                    event_name = Some(rest.trim().to_string());
                }
                // other fields (id, retry, comments starting with ':') ignored
            }
        }
    }
}

pub struct SseEvent {
    pub event: String,
    pub data: String,
}

// We depend on async-stream for brevity; add to Cargo.toml.
