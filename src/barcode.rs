use qrcode::QrCode;

pub struct BarPattern {
    pub modules: Vec<bool>, // true = black bar, false = white space
}

pub fn generate_qr_matrix(data: &str) -> Option<(usize, Vec<bool>)> {
    match QrCode::new(data.as_bytes()) {
        Ok(code) => {
            let width = code.width();
            let colors: Vec<bool> = code
                .to_colors()
                .into_iter()
                .map(|c| c == qrcode::Color::Dark)
                .collect();
            Some((width, colors))
        }
        Err(_) => None,
    }
}

/// Narrowest-bar width in dots that fits `module_count` modules along the box.
pub fn bar_dots(module_count: usize, along_dots: u32) -> u32 {
    (along_dots / module_count.max(1) as u32).clamp(1, 10)
}

/// Bar height in dots; the human-readable line takes the rest of the box.
pub fn bar_height_dots(across_dots: u32, show_text: bool) -> u32 {
    let height = if show_text { across_dots * 4 / 5 } else { across_dots };
    height.max(1)
}

/// Dots per QR module that fit the box.
pub fn qr_scale(data: &str, box_dots: u32) -> u32 {
    let modules = generate_qr_matrix(data).map_or(21, |(width, _)| width as u32);
    (box_dots / modules).clamp(1, 10)
}

// Code 39 patterns: 5 bars and 4 spaces in order, n = narrow, w = wide (3 modules)
fn code39_char_pattern(c: char) -> Option<&'static str> {
    Some(match c.to_ascii_uppercase() {
        '0' => "nnnwwnwnn", '1' => "wnnwnnnnw", '2' => "nnwwnnnnw", '3' => "wnwwnnnnn",
        '4' => "nnnwwnnnw", '5' => "wnnwwnnnn", '6' => "nnwwwnnnn", '7' => "nnnwnnwnw",
        '8' => "wnnwnnwnn", '9' => "nnwwnnwnn", 'A' => "wnnnnwnnw", 'B' => "nnwnnwnnw",
        'C' => "wnwnnwnnn", 'D' => "nnnnwwnnw", 'E' => "wnnnwwnnn", 'F' => "nnwnwwnnn",
        'G' => "nnnnnwwnw", 'H' => "wnnnnwwnn", 'I' => "nnwnnwwnn", 'J' => "nnnnwwwnn",
        'K' => "wnnnnnnww", 'L' => "nnwnnnnww", 'M' => "wnwnnnnwn", 'N' => "nnnnwnnww",
        'O' => "wnnnwnnwn", 'P' => "nnwnwnnwn", 'Q' => "nnnnnnwww", 'R' => "wnnnnnwwn",
        'S' => "nnwnnnwwn", 'T' => "nnnnwnwwn", 'U' => "wwnnnnnnw", 'V' => "nwwnnnnnw",
        'W' => "wwwnnnnnn", 'X' => "nwnnwnnnw", 'Y' => "wwnnwnnnn", 'Z' => "nwwnwnnnn",
        '-' => "nwnnnnwnw", '.' => "wwnnnnwnn", ' ' => "nwwnnnwnn", '*' => "nwnnwnwnn",
        '$' => "nwnwnwnnn", '/' => "nwnwnnnwn", '+' => "nwnnnwnwn", '%' => "nnnwnwnwn",
        _ => return None,
    })
}

pub fn is_code39_char(c: char) -> bool {
    c != '*' && code39_char_pattern(c).is_some()
}

pub fn generate_code39(text: &str) -> BarPattern {
    let mut modules = Vec::new();
    let content = format!("*{}*", text.trim_matches('*'));

    for (char_idx, ch) in content.chars().enumerate() {
        if char_idx > 0 {
            // Inter-character narrow space
            modules.push(false);
        }
        if let Some(pattern) = code39_char_pattern(ch) {
            for (idx, element) in pattern.chars().enumerate() {
                let is_bar = idx % 2 == 0;
                let count = if element == 'w' { 3 } else { 1 };
                for _ in 0..count {
                    modules.push(is_bar);
                }
            }
        }
    }

    BarPattern { modules }
}

// Code 128 widths (6 elements per symbol: bar, space, bar, space, bar, space)
const CODE128_PATTERNS: [[u8; 6]; 107] = [
    [2, 1, 2, 2, 2, 2], // 0: ' '
    [2, 2, 2, 1, 2, 2], // 1: '!'
    [2, 2, 2, 2, 2, 1], // 2: '"'
    [1, 2, 1, 2, 2, 3], // 3: '#'
    [1, 2, 1, 3, 2, 2], // 4: '$'
    [1, 3, 1, 2, 2, 2], // 5: '%'
    [1, 2, 2, 2, 1, 3], // 6: '&'
    [1, 2, 2, 3, 1, 2], // 7: '\''
    [1, 3, 2, 2, 1, 2], // 8: '('
    [2, 2, 1, 2, 1, 3], // 9: ')'
    [2, 2, 1, 3, 1, 2], // 10: '*'
    [2, 3, 1, 2, 1, 2], // 11: '+'
    [1, 1, 2, 2, 3, 2], // 12: ','
    [1, 2, 2, 1, 3, 2], // 13: '-'
    [1, 2, 2, 2, 3, 1], // 14: '.'
    [1, 1, 3, 2, 2, 2], // 15: '/'
    [1, 2, 3, 1, 2, 2], // 16: '0'
    [1, 2, 3, 2, 2, 1], // 17: '1'
    [2, 2, 3, 2, 1, 1], // 18: '2'
    [2, 2, 1, 1, 3, 2], // 19: '3'
    [2, 2, 1, 2, 3, 1], // 20: '4'
    [2, 1, 3, 2, 1, 2], // 21: '5'
    [2, 2, 3, 1, 1, 2], // 22: '6'
    [3, 1, 2, 1, 3, 1], // 23: '7'
    [3, 1, 1, 2, 2, 2], // 24: '8'
    [3, 2, 1, 1, 2, 2], // 25: '9'
    [3, 2, 1, 2, 2, 1], // 26: ':'
    [3, 1, 2, 2, 1, 2], // 27: ';'
    [3, 2, 2, 1, 1, 2], // 28: '<'
    [3, 2, 2, 2, 1, 1], // 29: '='
    [2, 1, 2, 1, 2, 3], // 30: '>'
    [2, 1, 2, 3, 2, 1], // 31: '?'
    [2, 3, 2, 1, 2, 1], // 32: '@'
    [1, 1, 1, 3, 2, 3], // 33: 'A'
    [1, 3, 1, 1, 2, 3], // 34: 'B'
    [1, 3, 1, 3, 2, 1], // 35: 'C'
    [1, 1, 2, 3, 1, 3], // 36: 'D'
    [1, 3, 2, 1, 1, 3], // 37: 'E'
    [1, 3, 2, 3, 1, 1], // 38: 'F'
    [2, 1, 1, 3, 1, 3], // 39: 'G'
    [2, 3, 1, 1, 1, 3], // 40: 'H'
    [2, 3, 1, 3, 1, 1], // 41: 'I'
    [1, 1, 2, 1, 3, 3], // 42: 'J'
    [1, 1, 2, 3, 3, 1], // 43: 'K'
    [1, 3, 2, 1, 3, 1], // 44: 'L'
    [1, 1, 3, 1, 2, 3], // 45: 'M'
    [1, 1, 3, 3, 2, 1], // 46: 'N'
    [1, 3, 3, 1, 2, 1], // 47: 'O'
    [3, 1, 3, 1, 2, 1], // 48: 'P'
    [2, 1, 1, 3, 3, 1], // 49: 'Q'
    [2, 3, 1, 1, 3, 1], // 50: 'R'
    [2, 1, 3, 1, 1, 3], // 51: 'S'
    [2, 1, 3, 3, 1, 1], // 52: 'T'
    [2, 1, 3, 1, 3, 1], // 53: 'U'
    [3, 1, 1, 1, 2, 3], // 54: 'V'
    [3, 1, 1, 3, 2, 1], // 55: 'W'
    [3, 3, 1, 1, 2, 1], // 56: 'X'
    [3, 1, 2, 1, 1, 3], // 57: 'Y'
    [3, 1, 2, 3, 1, 1], // 58: 'Z'
    [3, 3, 2, 1, 1, 1], // 59: '['
    [3, 1, 4, 1, 1, 1], // 60: '\\'
    [2, 2, 1, 4, 1, 1], // 61: ']'
    [4, 3, 1, 1, 1, 1], // 62: '^'
    [1, 1, 1, 2, 2, 4], // 63: '_'
    [1, 1, 1, 4, 2, 2], // 64: '`'
    [1, 2, 1, 1, 2, 4], // 65: 'a'
    [1, 2, 1, 4, 2, 1], // 66: 'b'
    [1, 4, 1, 1, 2, 2], // 67: 'c'
    [1, 4, 1, 2, 2, 1], // 68: 'd'
    [1, 1, 2, 2, 1, 4], // 69: 'e'
    [1, 1, 2, 4, 1, 2], // 70: 'f'
    [1, 2, 2, 1, 1, 4], // 71: 'g'
    [1, 2, 2, 4, 1, 1], // 72: 'h'
    [1, 4, 2, 1, 1, 2], // 73: 'i'
    [1, 4, 2, 2, 1, 1], // 74: 'j'
    [2, 4, 1, 2, 1, 1], // 75: 'k'
    [2, 2, 1, 1, 1, 4], // 76: 'l'
    [4, 1, 3, 1, 1, 1], // 77: 'm'
    [2, 4, 1, 1, 1, 2], // 78: 'n'
    [1, 3, 4, 1, 1, 1], // 79: 'o'
    [1, 1, 1, 2, 4, 2], // 80: 'p'
    [1, 2, 1, 1, 4, 2], // 81: 'q'
    [1, 2, 1, 2, 4, 1], // 82: 'r'
    [1, 1, 4, 2, 1, 2], // 83: 's'
    [1, 2, 4, 1, 1, 2], // 84: 't'
    [1, 2, 4, 2, 1, 1], // 85: 'u'
    [4, 1, 1, 2, 1, 2], // 86: 'v'
    [4, 2, 1, 1, 1, 2], // 87: 'w'
    [4, 2, 1, 2, 1, 1], // 88: 'x'
    [2, 1, 2, 1, 4, 1], // 89: 'y'
    [2, 1, 4, 1, 2, 1], // 90: 'z'
    [4, 1, 2, 1, 2, 1], // 91: '{'
    [1, 1, 1, 1, 4, 3], // 92: '|'
    [1, 1, 1, 3, 4, 1], // 93: '}'
    [1, 3, 1, 1, 4, 1], // 94: '~'
    [1, 1, 4, 1, 1, 3], // 95: DEL
    [1, 1, 4, 3, 1, 1], // 96: FNC3
    [4, 1, 1, 1, 1, 3], // 97: FNC2
    [4, 1, 1, 3, 1, 1], // 98: SHIFT
    [1, 1, 3, 1, 4, 1], // 99: CODE C
    [1, 1, 4, 1, 3, 1], // 100: CODE B
    [3, 1, 1, 1, 4, 1], // 101: FNC4
    [4, 1, 1, 1, 3, 1], // 102: FNC1
    [2, 1, 1, 4, 1, 2], // 103: START A
    [2, 1, 1, 2, 1, 4], // 104: START B
    [2, 1, 1, 2, 3, 2], // 105: START C
    [2, 3, 3, 1, 1, 1], // 106: STOP
];

pub fn generate_code128(text: &str) -> BarPattern {
    let mut modules = Vec::new();
    let mut symbols = Vec::new();

    // Start Code B (symbol 104)
    symbols.push(104usize);
    let mut checksum_sum: usize = 104;

    for (pos, b) in text.bytes().enumerate() {
        let code_val = if b >= 32 && b <= 126 {
            (b - 32) as usize
        } else {
            0 // fallback space
        };
        symbols.push(code_val);
        checksum_sum += code_val * (pos + 1);
    }

    let checksum_val = checksum_sum % 103;
    symbols.push(checksum_val);
    symbols.push(106); // Stop symbol

    for (i, &sym) in symbols.iter().enumerate() {
        let widths = CODE128_PATTERNS[sym];
        for (idx, &w) in widths.iter().enumerate() {
            let is_bar = idx % 2 == 0;
            for _ in 0..w {
                modules.push(is_bar);
            }
        }
        // Stop symbol has an extra 2-module termination bar at the very end
        if i == symbols.len() - 1 {
            modules.push(true);
            modules.push(true);
        }
    }

    BarPattern { modules }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code39_character_has_three_wide_elements() {
        let alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. *";
        let mut seen = std::collections::HashSet::new();
        for c in alphabet.chars() {
            let pattern = code39_char_pattern(c).unwrap();
            assert_eq!(pattern.len(), 9, "{c}");
            assert_eq!(pattern.matches('w').count(), 3, "{c}");
            assert!(seen.insert(pattern), "duplicate pattern for {c}");
        }
    }

    #[test]
    fn code39_has_expected_length() {
        // 3 characters with start/stop: 5 symbols of 15 modules plus 4 gaps
        assert_eq!(generate_code39("A1Z").modules.len(), 5 * 15 + 4);
    }

    #[test]
    fn code128_has_expected_length_without_quiet_zone() {
        // start + 2 chars + check, 11 modules each, plus 13-module stop
        let pattern = generate_code128("AB");
        assert_eq!(pattern.modules.len(), 4 * 11 + 13);
        assert!(pattern.modules[0]);
        assert!(*pattern.modules.last().unwrap());
    }

    #[test]
    fn bar_width_fits_box_and_never_reaches_zero() {
        assert_eq!(bar_dots(101, 320), 3);
        assert_eq!(bar_dots(1000, 320), 1);
        assert_eq!(bar_dots(10, 320), 10);
    }
}
