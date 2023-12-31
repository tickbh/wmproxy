// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/11/14 11:07:12

// cribbed to a large extent from log4rs

mod pattern;
mod proxy_record;

pub use self::pattern::*;
pub use self::proxy_record::*;



use std::{fmt, io};

pub mod writer;

#[allow(dead_code)]
#[cfg(windows)]
const NEWLINE: &'static str = "\r\n";

#[allow(dead_code)]
#[cfg(not(windows))]
const NEWLINE: &str = "\n";

/// A trait implemented by types that can serialize a `Record` into a
/// `Write`r.
///
/// `Encode`rs are commonly used by `Append`ers to format a log record for
/// output.
pub trait Encode: fmt::Debug + Send + Sync + 'static {
    /// Encodes the `Record` into bytes and writes them.
    fn encode(&self, w: &mut dyn Write, record: &ProxyRecord) -> io::Result<()>;
}

/// A text or background color.
#[allow(missing_docs)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

/// The style applied to text output.
///
/// Any fields set to `None` will be set to their default format, as defined
/// by the `Write`r.
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct Style {
    /// The text (or foreground) color.
    pub text: Option<Color>,
    /// The background color.
    pub background: Option<Color>,
    /// True if the text should have increased intensity.
    pub intense: Option<bool>,
}

impl Style {
    /// Returns a `Style` with all fields set to their defaults.
    pub fn new() -> Style {
        Style::default()
    }

    /// Sets the text color.
    pub fn text(&mut self, text: Color) -> &mut Style {
        self.text = Some(text);
        self
    }

    /// Sets the background color.
    pub fn background(&mut self, background: Color) -> &mut Style {
        self.background = Some(background);
        self
    }

    /// Sets the text intensity.
    pub fn intense(&mut self, intense: bool) -> &mut Style {
        self.intense = Some(intense);
        self
    }
}

/// A trait for types that an `Encode`r will write to.
///
/// It extends `std::io::Write` and adds some extra functionality.
pub trait Write: io::Write {
    /// Sets the output text style, if supported.
    ///
    /// `Write`rs should ignore any parts of the `Style` they do not support.
    ///
    /// The default implementation returns `Ok(())`. Implementations that do
    /// not support styling should do this as well.
    #[allow(unused_variables)]
    fn set_style(&mut self, style: &Style) -> io::Result<()> {
        Ok(())
    }
}

impl<'a, W: Write + ?Sized> Write for &'a mut W {
    fn set_style(&mut self, style: &Style) -> io::Result<()> {
        <W as Write>::set_style(*self, style)
    }
}

