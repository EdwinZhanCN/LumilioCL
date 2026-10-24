//! A scripted server for the transfer tests: it serves one content with an
//! optional `ETag`, honours ranges if told to, can break or stall an answer,
//! and records what it was asked and how many answers were open at once.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use futures_util::stream;
use futures_util::stream::StreamExt;

use super::transport::{ByteStream, Transport, TransportError, TransportFuture, TransportResponse};

/// How the next answer ends after the bytes it was asked for start.
#[derive(Clone, Copy, Debug)]
pub(super) enum Ending {
    Whole,
    /// A network error after this many bytes.
    BreakAfter(usize),
    /// No more bytes until dropped, after this many.
    StallAfter(usize),
}

/// One request the server answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Asked {
    pub(super) url: String,
    pub(super) start: u64,
    pub(super) end: Option<u64>,
    pub(super) validator: Option<String>,
    /// Whether the answer was the range asked for (`206`).
    pub(super) ranged: bool,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) content: Vec<u8>,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    /// Whether a range is answered with `206`, else with the whole `200`.
    pub(super) ranges: bool,
    /// Whether a whole answer says `Accept-Ranges: bytes`.
    pub(super) advertise: bool,
    /// Where a whole answer says it came from, as after a redirect.
    pub(super) redirect: Option<String>,
    /// Bytes per chunk of a body; 0 sends each answer as one chunk.
    pub(super) chunk: usize,
    /// A pause before each chunk.
    pub(super) delay: Duration,
    pub(super) endings: VecDeque<Ending>,
    pub(super) asked: Vec<Asked>,
    open: usize,
    most: usize,
}

#[derive(Clone, Default)]
pub(super) struct Server(Arc<Mutex<State>>);

impl Server {
    pub(super) fn new(content: &[u8], etag: Option<&str>, ranges: bool) -> Self {
        let server = Self::default();
        {
            let mut state = server.state();
            state.content = content.to_vec();
            state.etag = etag.map(str::to_owned);
            state.ranges = ranges;
            state.advertise = ranges;
        }
        server
    }

    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn then(&self, ending: Ending) -> &Self {
        self.state().endings.push_back(ending);
        self
    }

    /// Each request's starting byte and `If-Range`.
    pub(super) fn asked(&self) -> Vec<(u64, Option<String>)> {
        self.state()
            .asked
            .iter()
            .map(|asked| (asked.start, asked.validator.clone()))
            .collect()
    }

    pub(super) fn requests(&self) -> Vec<Asked> {
        self.state().asked.clone()
    }

    /// The most answers that were open at once.
    pub(super) fn most_open(&self) -> usize {
        self.state().most
    }

    /// `range`: the first byte and the end asked for, if any was.
    fn answer(
        &self,
        url: &str,
        range: Option<(u64, Option<u64>)>,
        validator: Option<&str>,
    ) -> TransportResponse {
        let (start, end) = range.unwrap_or((0, None));
        let mut state = self.state();
        let length = state.content.len();
        // As HTTP reads `If-Range`: a quoted tag that matches, or the date;
        // anything else is a date it cannot parse, so the file changed.
        let fresh = validator.is_none_or(|validator| {
            (validator.starts_with('"') && state.etag.as_deref() == Some(validator))
                || state.last_modified.as_deref() == Some(validator)
        });
        let ranged = range.is_some() && state.ranges && fresh;
        state.asked.push(Asked {
            url: url.to_owned(),
            start,
            end,
            validator: validator.map(str::to_owned),
            ranged,
        });
        let (from, to) = if ranged {
            (
                start as usize,
                end.map_or(length, |end| (end as usize).min(length)),
            )
        } else {
            (0, length)
        };
        let rest = state.content[from..to].to_vec();
        let ending = state.endings.pop_front().unwrap_or(Ending::Whole);
        state.open += 1;
        state.most = state.most.max(state.open);
        let mut headers = Vec::new();
        if let Some(etag) = &state.etag {
            headers.push(("etag".to_owned(), etag.clone()));
        }
        if let Some(date) = &state.last_modified {
            headers.push(("last-modified".to_owned(), date.clone()));
        }
        let status = if ranged {
            headers.push((
                "content-range".to_owned(),
                format!("bytes {from}-{}/{length}", to - 1),
            ));
            206
        } else {
            if state.advertise {
                headers.push(("accept-ranges".to_owned(), "bytes".to_owned()));
            }
            200
        };
        let body = body(self.clone(), &state, rest.clone(), ending);
        let response =
            TransportResponse::new(status, Some(rest.len() as u64), body).with_headers(headers);
        match &state.redirect {
            Some(redirect) if !ranged => response.with_url(redirect.clone()),
            _ => response.with_url(url),
        }
    }
}

/// Lowers the open count when an answer's body is dropped.
struct Open(Server);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.state().open -= 1;
    }
}

fn body(server: Server, state: &State, bytes: Vec<u8>, ending: Ending) -> ByteStream {
    let (bytes, tail): (Vec<u8>, Option<Result<Vec<u8>, TransportError>>) = match ending {
        Ending::Whole => (bytes, None),
        Ending::BreakAfter(count) => (
            bytes[..count].to_vec(),
            Some(Err(TransportError::transient("connection reset"))),
        ),
        Ending::StallAfter(count) => (bytes[..count].to_vec(), None),
    };
    let size = if state.chunk == 0 {
        bytes.len().max(1)
    } else {
        state.chunk
    };
    let delay = state.delay;
    let chunks: Vec<Vec<u8>> = bytes.chunks(size).map(<[u8]>::to_vec).collect();
    let open = Open(server);
    let paced = stream::iter(chunks).then(move |chunk| async move {
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        Ok(chunk)
    });
    let ended = paced.chain(stream::iter(tail));
    let stalled = matches!(ending, Ending::StallAfter(_));
    let tail: ByteStream = if stalled {
        Box::pin(ended.chain(stream::pending()))
    } else {
        Box::pin(ended)
    };
    // The guard lives as long as the body.
    Box::pin(tail.map(move |item| {
        let _ = &open;
        item
    }))
}

impl Transport for Server {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move { Ok(self.answer(source, None, None)) })
    }

    fn get_range<'a>(
        &'a self,
        source: &'a str,
        start: u64,
        end: Option<u64>,
        validator: Option<&'a str>,
    ) -> TransportFuture<'a> {
        Box::pin(async move { Ok(self.answer(source, Some((start, end)), validator)) })
    }
}
