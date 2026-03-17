use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use uuid::Uuid;

// Frame format: [u8 type][u32 len][JSON payload]
/// Maximum allowed message size (64 MB). Rejects frames that claim to be
/// larger than this to prevent OOM from corrupted or malicious peers.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

// Type tags for control messages
const MSG_SPAWN: u8 = 1;
const MSG_KILL: u8 = 2;
const MSG_RESIZE: u8 = 3;
const MSG_LIST: u8 = 4;
const MSG_HAS_SESSION: u8 = 5;
const MSG_SHUTDOWN: u8 = 6;
const MSG_OK: u8 = 10;
const MSG_ERROR: u8 = 11;
const MSG_LIST_RESP: u8 = 12;
const MSG_HAS_SESSION_RESP: u8 = 13;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpawnRequest {
    pub session_id: Uuid,
    pub cmd: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KillRequest {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResizeRequest {
    pub session_id: Uuid,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HasSessionRequest {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OkResponse {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    pub id: Uuid,
    pub alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListResponse {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HasSessionResponse {
    pub alive: bool,
}

/// All possible client-to-broker messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Spawn(SpawnRequest),
    Kill(KillRequest),
    Resize(ResizeRequest),
    List,
    HasSession(HasSessionRequest),
    Shutdown,
}

/// All possible broker-to-client messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    Ok(OkResponse),
    Error(ErrorResponse),
    List(ListResponse),
    HasSession(HasSessionResponse),
}

/// Encode a request into a length-prefixed frame.
pub fn encode_request(req: &Request) -> io::Result<Vec<u8>> {
    let (tag, payload) = match req {
        Request::Spawn(r) => (MSG_SPAWN, serde_json::to_vec(r).map_err(json_to_io)?),
        Request::Kill(r) => (MSG_KILL, serde_json::to_vec(r).map_err(json_to_io)?),
        Request::Resize(r) => (MSG_RESIZE, serde_json::to_vec(r).map_err(json_to_io)?),
        Request::List => (MSG_LIST, b"{}".to_vec()),
        Request::HasSession(r) => (MSG_HAS_SESSION, serde_json::to_vec(r).map_err(json_to_io)?),
        Request::Shutdown => (MSG_SHUTDOWN, b"{}".to_vec()),
    };
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(tag);
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Encode a response into a length-prefixed frame.
pub fn encode_response(resp: &Response) -> io::Result<Vec<u8>> {
    let (tag, payload) = match resp {
        Response::Ok(r) => (MSG_OK, serde_json::to_vec(r).map_err(json_to_io)?),
        Response::Error(r) => (MSG_ERROR, serde_json::to_vec(r).map_err(json_to_io)?),
        Response::List(r) => (MSG_LIST_RESP, serde_json::to_vec(r).map_err(json_to_io)?),
        Response::HasSession(r) => (
            MSG_HAS_SESSION_RESP,
            serde_json::to_vec(r).map_err(json_to_io)?,
        ),
    };
    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(tag);
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Convert a serde_json error into an io::Error.
fn json_to_io(e: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// Read a complete frame from a byte slice, returning (message, bytes_consumed).
/// Returns `None` if not enough data is available yet.
/// Returns an error if the declared length exceeds MAX_MESSAGE_SIZE.
fn read_frame(buf: &[u8]) -> Result<Option<(u8, Vec<u8>, usize)>, io::Error> {
    if buf.len() < 5 {
        return Ok(None);
    }
    let tag = buf[0];
    let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
        ));
    }
    let total = 5 + len;
    if buf.len() < total {
        return Ok(None);
    }
    Ok(Some((tag, buf[5..total].to_vec(), total)))
}

/// Decode a request from raw bytes. Returns (request, bytes_consumed).
pub fn decode_request(buf: &[u8]) -> Result<Option<(Request, usize)>, io::Error> {
    let Some((tag, payload, consumed)) = read_frame(buf)? else {
        return Ok(None);
    };
    let req = match tag {
        MSG_SPAWN => Request::Spawn(serde_json::from_slice(&payload)?),
        MSG_KILL => Request::Kill(serde_json::from_slice(&payload)?),
        MSG_RESIZE => Request::Resize(serde_json::from_slice(&payload)?),
        MSG_LIST => Request::List,
        MSG_HAS_SESSION => Request::HasSession(serde_json::from_slice(&payload)?),
        MSG_SHUTDOWN => Request::Shutdown,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown request tag: {}", tag),
            ))
        }
    };
    Ok(Some((req, consumed)))
}

/// Decode a response from raw bytes. Returns (response, bytes_consumed).
pub fn decode_response(buf: &[u8]) -> Result<Option<(Response, usize)>, io::Error> {
    let Some((tag, payload, consumed)) = read_frame(buf)? else {
        return Ok(None);
    };
    let resp = match tag {
        MSG_OK => Response::Ok(serde_json::from_slice(&payload)?),
        MSG_ERROR => Response::Error(serde_json::from_slice(&payload)?),
        MSG_LIST_RESP => Response::List(serde_json::from_slice(&payload)?),
        MSG_HAS_SESSION_RESP => Response::HasSession(serde_json::from_slice(&payload)?),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown response tag: {}", tag),
            ))
        }
    };
    Ok(Some((resp, consumed)))
}

// --- Async helpers for tokio UnixStream ---

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Send a request over an async Unix stream.
pub async fn send_request(stream: &mut UnixStream, req: &Request) -> io::Result<()> {
    let frame = encode_request(req)?;
    stream.write_all(&frame).await
}

/// Receive a response from an async Unix stream.
pub async fn recv_response(stream: &mut UnixStream) -> io::Result<Response> {
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).await?;
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
        ));
    }
    let mut payload = vec![0u8; len];
    if len > 0 {
        stream.read_exact(&mut payload).await?;
    }
    let mut full = Vec::with_capacity(5 + len);
    full.extend_from_slice(&header);
    full.extend_from_slice(&payload);
    match decode_response(&full)? {
        Some((resp, _)) => Ok(resp),
        None => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete response frame",
        )),
    }
}

/// Receive a request from an async Unix stream.
pub async fn recv_request(stream: &mut UnixStream) -> io::Result<Request> {
    let mut header = [0u8; 5];
    stream.read_exact(&mut header).await?;
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
        ));
    }
    let mut payload = vec![0u8; len];
    if len > 0 {
        stream.read_exact(&mut payload).await?;
    }
    let mut full = Vec::with_capacity(5 + len);
    full.extend_from_slice(&header);
    full.extend_from_slice(&payload);
    match decode_request(&full)? {
        Some((req, _)) => Ok(req),
        None => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete request frame",
        )),
    }
}

/// Send a response over an async Unix stream.
pub async fn send_response(stream: &mut UnixStream, resp: &Response) -> io::Result<()> {
    let frame = encode_response(resp)?;
    stream.write_all(&frame).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_spawn() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        let req = Request::Spawn(SpawnRequest {
            session_id: Uuid::nil(),
            cmd: "claude".to_string(),
            args: vec!["--continue".to_string()],
            cwd: "/tmp".to_string(),
            env,
            rows: 24,
            cols: 80,
        });
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn request_roundtrip_kill() {
        let req = Request::Kill(KillRequest {
            session_id: Uuid::nil(),
        });
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn request_roundtrip_resize() {
        let req = Request::Resize(ResizeRequest {
            session_id: Uuid::nil(),
            rows: 48,
            cols: 120,
        });
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn request_roundtrip_list() {
        let req = Request::List;
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn request_roundtrip_has_session() {
        let req = Request::HasSession(HasSessionRequest {
            session_id: Uuid::nil(),
        });
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn request_roundtrip_shutdown() {
        let req = Request::Shutdown;
        let encoded = encode_request(&req).unwrap();
        let (decoded, consumed) = decode_request(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, req);
    }

    #[test]
    fn response_roundtrip_ok() {
        let resp = Response::Ok(OkResponse {
            session_id: Uuid::nil(),
        });
        let encoded = encode_response(&resp).unwrap();
        let (decoded, consumed) = decode_response(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, resp);
    }

    #[test]
    fn response_roundtrip_error() {
        let resp = Response::Error(ErrorResponse {
            message: "something went wrong".to_string(),
        });
        let encoded = encode_response(&resp).unwrap();
        let (decoded, consumed) = decode_response(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, resp);
    }

    #[test]
    fn response_roundtrip_list() {
        let resp = Response::List(ListResponse {
            sessions: vec![
                SessionInfo {
                    id: Uuid::nil(),
                    alive: true,
                },
                SessionInfo {
                    id: Uuid::nil(),
                    alive: false,
                },
            ],
        });
        let encoded = encode_response(&resp).unwrap();
        let (decoded, consumed) = decode_response(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, resp);
    }

    #[test]
    fn response_roundtrip_has_session() {
        let resp = Response::HasSession(HasSessionResponse { alive: true });
        let encoded = encode_response(&resp).unwrap();
        let (decoded, consumed) = decode_response(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, resp);
    }

    #[test]
    fn decode_rejects_oversized_message() {
        // Craft a frame header claiming 128 MB payload (exceeds 64 MB limit)
        let huge_len: u32 = 128 * 1024 * 1024;
        let mut frame = vec![1u8]; // MSG_SPAWN tag
        frame.extend_from_slice(&huge_len.to_be_bytes());
        // Only need the header — the check happens before reading payload
        let result = decode_request(&frame);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[test]
    fn decode_request_returns_none_on_incomplete() {
        assert!(decode_request(&[]).unwrap().is_none());
        assert!(decode_request(&[1, 0, 0, 0]).unwrap().is_none());
        // Header says 100 bytes but only 5 available
        assert!(decode_request(&[1, 0, 0, 0, 100]).unwrap().is_none());
    }

    #[test]
    fn decode_request_rejects_unknown_tag() {
        let frame = [255, 0, 0, 0, 2, b'{', b'}'];
        let result = decode_request(&frame);
        assert!(result.is_err());
    }

    #[test]
    fn decode_response_rejects_unknown_tag() {
        let frame = [255, 0, 0, 0, 2, b'{', b'}'];
        let result = decode_response(&frame);
        assert!(result.is_err());
    }
}
