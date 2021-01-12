//! LIFX protocol labels. Labels are 32-byte UTF-8 strings

use std::convert::TryFrom;
use std::fmt;

use bytes::{Buf, BufMut};

use crate::ProtocolError;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Label(String);

impl Label {
    pub const MAX_LENGTH: usize = 32;

    /// Create a new `Label` from `str`.
    ///
    /// # Panics
    /// If `str` is longer than [`Label::MAX_LENGTH`]
    pub fn new<S: Into<String>>(str: S) -> Label {
        Label::try_from(str.into()).unwrap()
    }

    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.put_slice(self.0.as_bytes());
        for _ in 0..(Label::MAX_LENGTH - self.0.len()) {
            buf.put_u8(0);
        }
    }

    pub fn decode<B: Buf>(buf: &mut B) -> Result<Label, ProtocolError> {
        let mut str_bytes = Vec::with_capacity(Label::MAX_LENGTH);

        while str_bytes.len() < Label::MAX_LENGTH {
            let to_consume = Label::MAX_LENGTH - str_bytes.len();
            str_bytes.extend_from_slice(&buf.chunk()[..to_consume]);
            buf.advance(to_consume);
        }

        match String::from_utf8(str_bytes) {
            Ok(str) => Ok(Label(str.trim_end_matches(char::from(0)).into())),
            Err(_) => Err(ProtocolError::InvalidLabel),
        }
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for Label {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Label, ProtocolError> {
        if value.len() <= Label::MAX_LENGTH {
            Ok(Label(value))
        } else {
            Err(ProtocolError::InvalidLabel)
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
