//! SSE 流解析辅助 — 按字节缓冲、按行切分，避免 UTF-8 跨 chunk 截断

use futures_util::StreamExt;

use super::types::LlmError;

/// 逐条消费 SSE `data:` 载荷；遇到 [DONE] 或流结束返回
pub async fn for_each_sse_data<F>(resp: reqwest::Response, mut f: F) -> Result<(), LlmError>
where
    F: FnMut(&str) -> Result<(), LlmError>,
{
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| LlmError::Network(e.to_string()))?;
        buf.extend_from_slice(&chunk);
        // 换行字节不会出现在多字节 UTF-8 字符中间，按字节切行是安全的
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            let line = line.trim_end();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim_start();
                if data == "[DONE]" {
                    return Ok(());
                }
                if !data.is_empty() {
                    f(data)?;
                }
            }
        }
    }
    Ok(())
}
