//! Editing a complete message: decoded text for the editor and saved session changes.

use crate::{AppCore, dto::Part};
use anyhow::{Result, anyhow, bail};
use quena_body::{Body, BodyStore};
use quena_model::{Headers, SessionId};
use serde::Serialize;
use std::{io::Read, sync::Arc};

/// Limit the decoded input, rather than its compressed size. Never edit a partial body.
pub const EDIT_LIMIT: usize = 8 << 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableBody {
    pub text: String,
    pub charset: String,
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
}
