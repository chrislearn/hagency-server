// Cryptographically-secure random helpers backed by the browser's Web Crypto
// API (window.crypto.getRandomValues). The previous Math.random()
// implementation is *not* CSPRNG and made device IDs / generated passwords
// guessable.

const CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?";
const ALPHANUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

fn fill_random(buf: &mut [u8]) {
    let crypto = web_sys::window()
        .and_then(|w| w.crypto().ok())
        .expect("window.crypto is required for random generation");
    crypto
        .get_random_values_with_u8_array(buf)
        .expect("getRandomValues failed");
}

fn random_string(charset: &[u8], length: usize) -> String {
    // Rejection sampling avoids the modulo bias that `byte % charset.len()`
    // would introduce (e.g. for 62-char alphabets, byte values 248-255 would
    // map to the first 8 characters at slightly higher probability).
    let n = charset.len() as u8;
    let limit = (256 / charset.len() * charset.len()) as u16; // largest multiple of n that fits in a byte
    let mut out = String::with_capacity(length);
    let mut buf = [0u8; 64];
    while out.len() < length {
        fill_random(&mut buf);
        for &b in &buf {
            if (b as u16) < limit {
                out.push(charset[(b % n) as usize] as char);
                if out.len() == length {
                    break;
                }
            }
        }
    }
    out
}

pub fn generate_random_password() -> String {
    random_string(CHARS, 64)
}

pub fn generate_device_id() -> String {
    random_string(ALPHANUM, 16)
}
