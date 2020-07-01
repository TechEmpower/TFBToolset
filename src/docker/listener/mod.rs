use curl::easy::WriteError;

pub mod application;
pub mod build_container;
pub mod build_image;
pub mod build_network;
pub mod inspect_container;
pub mod simple;
pub mod verifier;

/// Simple accumulator; takes `data`, parses it as utf8, and pushes it onto
/// `string_buffer`.
pub fn accumulate(string_buffer: &mut String, data: &[u8]) -> Result<usize, WriteError> {
    if let Ok(bytes) = std::str::from_utf8(&data) {
        string_buffer.push_str(bytes);
    }

    Ok(data.len())
}
