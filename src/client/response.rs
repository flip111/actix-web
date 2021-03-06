use std::{fmt, str};
use std::rc::Rc;
use std::cell::UnsafeCell;

use bytes::Bytes;
use cookie::Cookie;
use futures::{Async, Poll, Stream};
use http::{HeaderMap, StatusCode, Version};
use http::header::{self, HeaderValue};

use httpmessage::HttpMessage;
use error::{CookieParseError, PayloadError};

use super::pipeline::Pipeline;


pub(crate) struct ClientMessage {
    pub status: StatusCode,
    pub version: Version,
    pub headers: HeaderMap<HeaderValue>,
    pub cookies: Option<Vec<Cookie<'static>>>,
}

impl Default for ClientMessage {

    fn default() -> ClientMessage {
        ClientMessage {
            status: StatusCode::OK,
            version: Version::HTTP_11,
            headers: HeaderMap::with_capacity(16),
            cookies: None,
        }
    }
}

/// An HTTP Client response
pub struct ClientResponse(Rc<UnsafeCell<ClientMessage>>, Option<Box<Pipeline>>);

impl HttpMessage for ClientResponse {
    /// Get the headers from the response.
    #[inline]
    fn headers(&self) -> &HeaderMap {
        &self.as_ref().headers
    }
}

impl ClientResponse {

    pub(crate) fn new(msg: ClientMessage) -> ClientResponse {
        ClientResponse(Rc::new(UnsafeCell::new(msg)), None)
    }

    pub(crate) fn set_pipeline(&mut self, pl: Box<Pipeline>) {
        self.1 = Some(pl);
    }

    #[inline]
    fn as_ref(&self) -> &ClientMessage {
        unsafe{ &*self.0.get() }
    }

    #[inline]
    #[cfg_attr(feature = "cargo-clippy", allow(mut_from_ref))]
    fn as_mut(&self) -> &mut ClientMessage {
        unsafe{ &mut *self.0.get() }
    }

    /// Get the HTTP version of this response.
    #[inline]
    pub fn version(&self) -> Version {
        self.as_ref().version
    }

    /// Get the status from the server.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.as_ref().status
    }

    /// Load request cookies.
    pub fn cookies(&self) -> Result<&Vec<Cookie<'static>>, CookieParseError> {
        if self.as_ref().cookies.is_none() {
            let msg = self.as_mut();
            let mut cookies = Vec::new();
            if let Some(val) = msg.headers.get(header::COOKIE) {
                let s = str::from_utf8(val.as_bytes())
                    .map_err(CookieParseError::from)?;
                for cookie in s.split("; ") {
                    cookies.push(Cookie::parse_encoded(cookie)?.into_owned());
                }
            }
            msg.cookies = Some(cookies)
        }
        Ok(self.as_ref().cookies.as_ref().unwrap())
    }

    /// Return request cookie.
    pub fn cookie(&self, name: &str) -> Option<&Cookie> {
        if let Ok(cookies) = self.cookies() {
            for cookie in cookies {
                if cookie.name() == name {
                    return Some(cookie)
                }
            }
        }
        None
    }
}

impl fmt::Debug for ClientResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = write!(
            f, "\nClientResponse {:?} {}\n", self.version(), self.status());
        let _ = write!(f, "  headers:\n");
        for key in self.headers().keys() {
            let vals: Vec<_> = self.headers().get_all(key).iter().collect();
            if vals.len() > 1 {
                let _ = write!(f, "    {:?}: {:?}\n", key, vals);
            } else {
                let _ = write!(f, "    {:?}: {:?}\n", key, vals[0]);
            }
        }
        res
    }
}

/// Future that resolves to a complete request body.
impl Stream for ClientResponse {
    type Item = Bytes;
    type Error = PayloadError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(ref mut pl) = self.1 {
            pl.poll()
        } else {
            Ok(Async::Ready(None))
        }
    }
}
