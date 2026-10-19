// SPDX-License-Identifier: GPL-2.0-only
//! Characters a commit message, a cover letter or a document must not carry.

pub const FORBIDDEN_R16: &[(char, &str)] = &[
    ('\u{2014}', "em-dash U+2014"),
    ('\u{2013}', "en-dash U+2013"),
    ('\u{2192}', "arrow U+2192"),
    ('\u{2018}', "left-single-quote U+2018"),
    ('\u{2019}', "right-single-quote U+2019"),
    ('\u{201C}', "left-double-quote U+201C"),
    ('\u{201D}', "right-double-quote U+201D"),
];
