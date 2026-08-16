//! UTF-8 safe string trim/keep helpers.
//!
//! Byte-index slices (`&s[..n]`, `s[s.len()-k..]`) panic when `n` lands mid-char
//! (common after `from_utf8_lossy` inserts `�`, or Windows/USB paths with non-ASCII).
//! That panic was killing the whole Companion process (no per-thread catch).

/// Largest char boundary ≤ `i` (or 0).
pub fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smallest char boundary ≥ `i` (or `s.len()`).
pub fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Truncate to at most `max_bytes` (on a char boundary), appending `…` when cut.
pub fn trunc(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let end = floor_char_boundary(s, max_bytes);
    if end == 0 {
        "…".into()
    } else {
        format!("{}…", &s[..end])
    }
}

/// Keep only the last ~`keep` bytes of `s`, aligned to a char boundary.
pub fn keep_last(s: &mut String, keep: usize) {
    if s.len() <= keep {
        return;
    }
    let start = ceil_char_boundary(s, s.len() - keep);
    if start >= s.len() {
        s.clear();
        return;
    }
    s.replace_range(..start, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trunc_mid_multibyte_does_not_panic() {
        // "é" is 2 bytes; cutting at 1 must not panic.
        let s = "aébc";
        let t = trunc(s, 2); // would panic with &s[..2]
        assert!(t.ends_with('…') || t == "aé" || t.starts_with('a'));
        let _ = trunc("Njörðr — seas…", 8);
        let _ = trunc("CMPCONFIG ssid=café&fw=0.8.180-sha256", 20);
    }

    #[test]
    fn keep_last_mid_multibyte_does_not_panic() {
        let mut s = "aaa".to_string();
        s.push('€'); // 3 bytes
        s.push_str("bbbbbbbb");
        // Force a keep window that would land inside € if we used len-keep raw.
        let keep_n = s.len() - 1;
        keep_last(&mut s, keep_n);
        assert!(!s.is_empty());
        // Lossy replacement char U+FFFD is 3 bytes — classic OTA rx trim footgun.
        let mut noisy = "x".repeat(5000);
        noisy.push('\u{FFFD}');
        noisy.push_str(&"y".repeat(500));
        keep_last(&mut noisy, 1024);
        assert!(noisy.len() <= 1024 + 4);
    }
}
