use std::path::PathBuf;
use std::fmt::{Debug, Display, Formatter};

use proc_macro2::Span;

#[derive(Eq, Hash, PartialEq)]
pub struct Loc<'a>(&'a PathBuf, usize, usize);
//                 file_path,   line,  column

impl<'a > Loc<'a> {
    #[inline(always)]
    pub fn from_span(file_path: &'a PathBuf, span: &Span) -> Self {
        let linecol = span.start();
        Loc(file_path, linecol.line, linecol.column)
    }
}

impl Display for Loc<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{f}:{l}:{c}", f = self.0.display(), l = self.1, c = self.2)
    }
}

impl Debug for Loc<'_> {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}
