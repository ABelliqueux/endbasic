// EndBASIC
// Copyright 2025 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Support for bitmap fonts directly rendered onto an LCD.

use crate::gfx::lcd::LcdSize;
use std::collections::HashMap;

mod font_5x8;
pub(crate) use font_5x8::FONT_5X8;

mod font_16x16;
pub(crate) use font_16x16::FONT_16X16;

/// Representation of a font.
pub struct Font {
    /// The name of the font.
    pub name: &'static str,

    /// The size of a single glyph, in pixels.
    pub glyph_size: LcdSize,

    /// The number of bytes in every glyph row.
    pub stride: usize,

    /// The bitmap data for the font.
    pub data: &'static [u8],
}

impl Font {
    /// Returns the raw font data for `ch`.
    ///
    /// Each entry in the array corresponds to a row of pixels and is a bitmask indicating which
    /// pixels to turn on.
    pub(crate) fn glyph(&self, mut ch: char) -> &'static [u8] {
        if !(' '..='~').contains(&ch) {
            // TODO(jmmv): Would be nicer to draw an empty box, much like how unknown Unicode
            // characters are typically displayed.
            ch = '?';
        }
        let height = self.glyph_size.height * self.stride;
        let offset = ((ch as usize) - (' ' as usize)) * height;
        debug_assert!(offset < (self.data.len() + height));
        &self.data[offset..offset + height]
    }
}

/// Registry of all available fonts.
pub type Fonts = HashMap<&'static str, &'static Font>;

/// Obtains a mapping of all available fonts.
pub fn all_fonts() -> Fonts {
    let mut fonts = Fonts::default();
    fonts.insert(FONT_5X8.name, &FONT_5X8);
    fonts.insert(FONT_16X16.name, &FONT_16X16);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_glyph_printable() {
        let font = &FONT_5X8;

        let offset = (usize::from(b'a') - usize::from(b' ')) * 8;
        let expected = &font.data[offset..offset + 8];

        let data = font.glyph('a');
        assert_eq!(expected, data);
    }

    #[test]
    fn test_font_glyph_non_printable() {
        let font = &FONT_5X8;

        let offset = (usize::from(b'?') - usize::from(b' ')) * 8;
        let expected = &font.data[offset..offset + 8];

        let data = font.glyph(char::from(30));
        assert_eq!(expected, data);
    }
}
