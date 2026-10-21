//! The encoding of a source map.
//!
//! The positions are written as variable length numbers, the way the source
//! map format asks for.

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn encode_vlq(value: i64, out: &mut String) {
    let mut value = if value < 0 {
        ((-value as u64) << 1) | 1
    } else {
        (value as u64) << 1
    };
    loop {
        let mut digit = (value & 31) as u8;
        value >>= 5;
        if value > 0 {
            digit |= 32;
        }
        out.push(BASE64[digit as usize] as char);
        if value == 0 {
            break;
        }
    }
}

pub(crate) fn base64(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut index = 0;
    while index + 3 <= input.len() {
        let value = ((input[index] as u32) << 16)
            | ((input[index + 1] as u32) << 8)
            | input[index + 2] as u32;
        out.push(BASE64[((value >> 18) & 63) as usize] as char);
        out.push(BASE64[((value >> 12) & 63) as usize] as char);
        out.push(BASE64[((value >> 6) & 63) as usize] as char);
        out.push(BASE64[(value & 63) as usize] as char);
        index += 3;
    }

    match input.len() - index {
        1 => {
            let value = (input[index] as u32) << 16;
            out.push(BASE64[((value >> 18) & 63) as usize] as char);
            out.push(BASE64[((value >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let value = ((input[index] as u32) << 16) | ((input[index + 1] as u32) << 8);
            out.push(BASE64[((value >> 18) & 63) as usize] as char);
            out.push(BASE64[((value >> 12) & 63) as usize] as char);
            out.push(BASE64[((value >> 6) & 63) as usize] as char);
            out.push('=');
        }
        _ => {}
    }

    out
}
