//! Shared SSE (Server-Sent Events) parsing for OpenAI-compatible streaming
//! endpoints.  Used by both `llama_cpp` and `api_key` providers.

use crate::ProviderError;
use eagent_contracts::provider::{FinishReason, ProviderEvent};
use futures_util::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::warn;

/// Read an SSE response body as a true byte stream and emit [`ProviderEvent`]s
/// through the channel as data arrives.
///
/// Each `data:` line is parsed individually.  `data: [DONE]` marks the end of
/// the stream.
pub async fn read_sse_stream(
    response: reqwest::Response,
    tx: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<(), ProviderError> {
    let mut buffer = String::new();

    // Accumulate tool call state across deltas.
    let mut tool_calls: HashMap<String, (String, String)> = HashMap::new();

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| ProviderError::Internal(format!("failed to read response chunk: {e}")))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process all complete lines in the buffer
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            if line == "data: [DONE]" {
                let _ = tx.send(ProviderEvent::Completion {
                    finish_reason: FinishReason::Stop,
                });
                return Ok(());
            }
            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Err(e) = parse_sse_data(json_str, tx, &mut tool_calls) {
                    warn!(target: "eagent::sse", "failed to parse SSE data: {e}");
                }
            }
        }
    }

    // If we get here without [DONE], still send a completion event.
    let _ = tx.send(ProviderEvent::Completion {
        finish_reason: FinishReason::Stop,
    });
    Ok(())
}

/// Parse a single SSE `data:` JSON payload and emit events.
pub fn parse_sse_data(
    json_str: &str,
    tx: &mpsc::UnboundedSender<ProviderEvent>,
    tool_calls: &mut HashMap<String, (String, String)>,
) -> Result<(), String> {
    let v: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;

    let choice = &v["choices"][0];
    let delta = &choice["delta"];

    // -- text token delta ---------------------------------------------------
    if let Some(text) = delta["content"].as_str() {
        if !text.is_empty() {
            let _ = tx.send(ProviderEvent::TokenDelta {
                text: text.to_string(),
            });
        }
    }

    // -- tool call deltas ---------------------------------------------------
    if let Some(tcs) = delta["tool_calls"].as_array() {
        for tc in tcs {
            let idx = tc["index"].as_u64().unwrap_or(0);
            let id = tc["id"].as_str().unwrap_or("").to_string();

            let func = &tc["function"];
            let name = func["name"].as_str().unwrap_or("").to_string();
            let args_frag = func["arguments"].as_str().unwrap_or("").to_string();

            // Derive a stable id: the server may send `id` only on the first
            // delta for each tool call index, so fall back to "tc_{idx}".
            let effective_id = if !id.is_empty() {
                id.clone()
            } else {
                format!("tc_{idx}")
            };

            if let Some((_n, accumulated)) = tool_calls.get_mut(&effective_id) {
                // Continuation delta.
                accumulated.push_str(&args_frag);
                let _ = tx.send(ProviderEvent::ToolCallDelta {
                    id: effective_id.clone(),
                    params_partial: args_frag,
                });
            } else {
                // First time we see this tool call id.
                tool_calls.insert(effective_id.clone(), (name.clone(), args_frag.clone()));
                let _ = tx.send(ProviderEvent::ToolCallStart {
                    id: effective_id.clone(),
                    name: name.clone(),
                    params_partial: args_frag,
                });
            }
        }
    }

    // -- finish reason ------------------------------------------------------
    if let Some(reason_str) = choice["finish_reason"].as_str() {
        let finish_reason = match reason_str {
            "stop" => FinishReason::Stop,
            "tool_calls" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        };

        // Emit ToolCallComplete for any accumulated tool calls.
        if finish_reason == FinishReason::ToolCalls {
            for (id, (name, args)) in tool_calls.drain() {
                let params: Value = serde_json::from_str(&args).unwrap_or(Value::Null);
                let _ = tx.send(ProviderEvent::ToolCallComplete { id, name, params });
            }
        }

        let _ = tx.send(ProviderEvent::Completion { finish_reason });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_token_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::TokenDelta { text } => assert_eq!(text, "Hello"),
            other => panic!("expected TokenDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_finish_reason_stop() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_finish_reason_length() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Length);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_start_and_delta() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();

        let data1 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"pa"}}]}}]}"#;
        parse_sse_data(data1, &tx, &mut tc).unwrap();

        let data2 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"arguments":"th\":\"/tmp\"}"}}]}}]}"#;
        parse_sse_data(data2, &tx, &mut tc).unwrap();

        drop(tx);

        let e1 = rx.blocking_recv().unwrap();
        match e1 {
            ProviderEvent::ToolCallStart { id, name, .. } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
            }
            other => panic!("expected ToolCallStart, got {:?}", other),
        }

        let e2 = rx.blocking_recv().unwrap();
        match e2 {
            ProviderEvent::ToolCallDelta { id, .. } => {
                assert_eq!(id, "call_1");
            }
            other => panic!("expected ToolCallDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_complete_on_finish() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();

        let data1 = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_2","function":{"name":"ls","arguments":"{\"dir\":\".\"}"}}]}}]}"#;
        parse_sse_data(data1, &tx, &mut tc).unwrap();

        let data2 = r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#;
        parse_sse_data(data2, &tx, &mut tc).unwrap();

        drop(tx);

        // ToolCallStart
        let _ = rx.blocking_recv().unwrap();
        // ToolCallComplete
        let e2 = rx.blocking_recv().unwrap();
        match e2 {
            ProviderEvent::ToolCallComplete { id, name, params } => {
                assert_eq!(id, "call_2");
                assert_eq!(name, "ls");
                assert_eq!(params["dir"], ".");
            }
            other => panic!("expected ToolCallComplete, got {:?}", other),
        }
        // Completion
        let e3 = rx.blocking_recv().unwrap();
        match e3 {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::ToolCalls);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_ignores_empty_content() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{"content":""}}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        assert!(rx.blocking_recv().is_none());
    }

    #[test]
    fn parse_sse_invalid_json_returns_error() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let result = parse_sse_data("not json", &tx, &mut tc);
        assert!(result.is_err());
    }

    #[test]
    fn parse_sse_content_filter_finish_reason() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::ContentFilter);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_unknown_finish_reason_defaults_to_stop() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut tc = HashMap::new();
        let data = r#"{"choices":[{"delta":{},"finish_reason":"something_new"}]}"#;
        parse_sse_data(data, &tx, &mut tc).unwrap();
        drop(tx);

        let event = rx.blocking_recv().unwrap();
        match event {
            ProviderEvent::Completion { finish_reason } => {
                assert_eq!(finish_reason, FinishReason::Stop);
            }
            other => panic!("expected Completion, got {:?}", other),
        }
    }
}
