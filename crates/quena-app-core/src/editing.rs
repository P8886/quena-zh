//! Editing a complete message: decoded text for the editor and saved session changes.

use crate::{AppCore, dto::Part};
use anyhow::{Result, anyhow, bail};
use quena_body::{Body, BodyStore};
use quena_model::{Headers, HttpVersion, RequestHead, ResponseHead, SessionId, SessionKind, flags};
use serde::{Deserialize, Serialize};
use std::{io::Read, sync::Arc};

/// Limit the decoded input, rather than its compressed size. Never edit a partial body.
pub const EDIT_LIMIT: usize = 8 << 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableBody {
    pub text: String,
    pub charset: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MessageEdit {
    pub head_text: Option<String>,
    pub body_text: Option<String>,
    pub body_charset: Option<String>,
    pub body_file: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionEdit {
    pub request: Option<MessageEdit>,
    pub response: Option<MessageEdit>,
}

/// Text edits are decoded input. Send it without Content-Encoding; untouched bodies keep
/// their exact original bytes and headers, including gzip/br and their framing.
pub(crate) fn text_body(store: &Arc<BodyStore>, text: &str, charset: Option<&str>, headers: &mut Headers) -> Body {
    let (bytes, ct) = quena_body::text::encode_edited(text, headers.get("content-type"), charset);
    if let Some(ct) = ct {
        headers.set("Content-Type", ct);
    }
    headers.remove("content-encoding");
    store.store_bytes(&bytes)
}

fn replacement(store: &Arc<BodyStore>, edit: &MessageEdit, headers: &mut Headers) -> Result<Option<Body>> {
    let body = if let Some(path) = &edit.body_file {
        Some(crate::rules::copy_to_body(store, std::fs::File::open(path)?).map_err(|e| anyhow!(e))?)
    } else {
        edit.body_text.as_ref().map(|text| text_body(store, text, edit.body_charset.as_deref(), headers))
    };
    if let Some(b) = &body {
        if b.is_truncated() {
            bail!("not enough storage to save the edited body");
        }
        headers.remove("transfer-encoding");
        headers.set("Content-Length", b.len().to_string());
    }
    Ok(body)
}

fn parse_head(text: &str) -> Result<(&str, Headers)> {
    let mut lines = text.lines();
    let first = lines.next().ok_or_else(|| anyhow!("missing message start line"))?.trim();
    let mut headers = Headers::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(|| anyhow!("invalid header line: {line}"))?;
        let (name, value) = (name.trim(), value.trim());
        http::header::HeaderName::from_bytes(name.as_bytes())?;
        http::header::HeaderValue::from_str(value)?;
        headers.push(name, value);
    }
    Ok((first, headers))
}

impl AppCore {
    /// Strictly decode a complete, bounded body. Search's tolerant prefix decoder must not
    /// be used here: a corrupt gzip trailer must not turn a partial body into an edit.
    pub fn editable_body(&self, id: SessionId, part: Part, charset: Option<&str>) -> Result<EditableBody> {
        let cap = self.capture();
        let (d, req, resp) = cap.bodies_stored(id).ok_or_else(|| anyhow!("session #{id} not found"))?;
        let (body, headers) = match part {
            Part::Request => (req, d.request.headers),
            Part::Response => (resp, d.response.ok_or_else(|| anyhow!("no response to edit"))?.headers),
        };
        if !body.is_complete() || body.is_truncated() {
            bail!("the body is incomplete or its recording was truncated");
        }
        let encodings = quena_body::decode::parse_encodings(headers.get("content-encoding").unwrap_or("")).map_err(|e| anyhow!("unsupported Content-Encoding: {e}"))?;
        let reader = quena_body::decode::decoding_reader(Box::new(body.stream(0, false)), &encodings);
        let mut bytes = Vec::new();
        reader.take((EDIT_LIMIT + 1) as u64).read_to_end(&mut bytes).map_err(|e| anyhow!("could not decode the complete body: {e}"))?;
        if bytes.len() > EDIT_LIMIT {
            bail!("the decoded body exceeds the 8 MiB editing limit; replace it with a file");
        }
        let enc = charset.and_then(quena_body::charset::for_label).unwrap_or_else(|| quena_body::charset::detect(headers.get("content-type"), &bytes).encoding);
        let (text, _, _) = enc.decode(&bytes);
        Ok(EditableBody { text: text.into_owned(), charset: enc.name().to_string() })
    }

    /// Save edits to a finished HTTP session, so replay and SAZ export use the same message.
    pub fn edit_session(&self, id: SessionId, edit: SessionEdit) -> Result<()> {
        let cap = self.capture();
        let d = cap.detail(id).ok_or_else(|| anyhow!("session #{id} not found"))?;
        if !d.summary.state.is_final() || cap.live(id).is_some() || d.summary.kind != SessionKind::Http {
            bail!("only finished HTTP sessions can be unlocked for editing");
        }
        let mut req = d.request;
        let mut resp = d.response;
        let mut req_body = None;
        let mut resp_body = None;
        if let Some(e) = edit.request {
            if let Some(text) = &e.head_text {
                let (first, headers) = parse_head(text)?;
                let fields: Vec<_> = first.split_whitespace().collect();
                if fields.len() != 3 || HttpVersion::parse(fields[2]).is_none() {
                    bail!("invalid request start line; use METHOD URL HTTP/1.1");
                }
                let (method, url, version) = (fields[0].to_string(), fields[1].to_string(), HttpVersion::parse(fields[2]).unwrap());
                http::Method::from_bytes(method.as_bytes())?;
                let uri: http::Uri = url.parse()?;
                if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.authority().is_none() {
                    bail!("the request URL must start with http:// or https://");
                }
                req = RequestHead { method, url, version, headers };
            }
            req_body = replacement(&cap.bodies, &e, &mut req.headers)?;
        }
        if let Some(e) = edit.response {
            let r = resp.as_mut().ok_or_else(|| anyhow!("no response to edit"))?;
            if let Some(text) = &e.head_text {
                let (first, headers) = parse_head(text)?;
                if first.split_whitespace().next().and_then(HttpVersion::parse).is_none() {
                    bail!("invalid response start line; use HTTP/1.1 200 OK");
                }
                let (version, status, reason) = quena_formats::raw::parse_status_line(first);
                http::StatusCode::from_u16(status)?;
                *r = ResponseHead { version, status, reason, headers };
            }
            resp_body = replacement(&cap.bodies, &e, &mut r.headers)?;
        }
        if !cap.update_detail(id, move |d| {
            d.request = req;
            d.response = resp;
            if let Some(b) = req_body { d.request_body = b.to_ref(); }
            if let Some(b) = resp_body { d.response_body = b.to_ref(); }
            d.summary.flags |= flags::TAMPERED;
        }) {
            bail!("session #{id} was removed before its edits could be saved");
        }
        Ok(())
    }
}
