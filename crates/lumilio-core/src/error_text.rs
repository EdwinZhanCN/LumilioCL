//! An error in words, with what caused it.

use std::error::Error;

/// The error and each cause behind it, outermost first: `a: b: c`. Network
/// errors say only "error sending request for url (…)" themselves; why (DNS,
/// TLS, refused, timed out) is in their causes, and that is what a technical
/// detail or a log needs. A cause already in the words so far is not repeated.
#[must_use]
pub fn with_causes(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        let words = next.to_string();
        if !words.is_empty() && !text.contains(&words) {
            text.push_str(": ");
            text.push_str(&words);
        }
        cause = next.source();
    }
    text
}

#[cfg(test)]
mod tests;
