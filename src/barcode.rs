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

// Code 39 patterns (5 bars, 4 spaces: 1 = wide (2 modules), 0 = narrow (1 module))
// Representation: 9 bits [bar, space, bar, space, bar, space, bar, space, bar]
fn code39_char_pattern(c: char) -> Option<[u8; 9]> {
    match c.to_ascii_uppercase() {
        '1' => Some([1, 0, 0, 0, 0, 0, 0, 0, 1]), // W-N-N-N-N-N-N-N-W
        '2' => Some([0, 0, 1, 0, 0, 0, 0, 0, 1]),
        '3' => Some([1, 0, 1, 0, 0, 0, 0, 0, 0]),
        '4' => Some([0, 0, 0, 0, 1, 0, 0, 0, 1]),
        '5' => Some([1, 0, 0, 0, 1, 0, 0, 0, 0]),
        '6' => Some([0, 0, 1, 0, 1, 0, 0, 0, 0]),
        '7' => Some([0, 0, 0, 0, 0, 0, 1, 0, 1]),
        '8' => Some([1, 0, 0, 0, 0, 0, 1, 0, 0]),
        '9' => Some([0, 0, 1, 0, 0, 0, 1, 0, 0]),
        '0' => Some([0, 0, 0, 0, 1, 0, 1, 0, 0]),
        'A' => Some([1, 0, 0, 0, 0, 0, 0, 1, 1]),
        'B' => Some([0, 0, 1, 0, 0, 0, 0, 1, 1]),
        'C' => Some([1, 0, 1, 0, 0, 0, 0, 1, 0]),
        'D' => Some([0, 0, 0, 0, 1, 0, 0, 1, 1]),
        'E' => Some([1, 0, 0, 0, 1, 0, 0, 1, 0]),
        'F' => Some([0, 0, 1, 0, 1, 0, 0, 1, 0]),
        'G' => Some([0, 0, 0, 0, 0, 0, 1, 1, 1]),
        'H' => Some([1, 0, 0, 0, 0, 0, 1, 1, 0]),
        'I' => Some([0, 0, 1, 0, 0, 0, 1, 1, 0]),
        'J' => Some([0, 0, 0, 0, 1, 0, 1, 1, 0]),
        'K' => Some([1, 0, 0, 0, 0, 1, 0, 0, 1]),
        'L' => Some([0, 0, 1, 0, 0, 1, 0, 0, 1]),
        'M' => Some([1, 0, 1, 0, 0, 1, 0, 0, 0]),
        'N' => Some([0, 0, 0, 0, 1, 1, 0, 0, 1]),
        'O' => Some([1, 0, 0, 0, 1, 1, 0, 0, 0]),
        'P' => Some([0, 0, 1, 0, 1, 1, 0, 0, 0]),
        'Q' => Some([0, 0, 0, 0, 0, 1, 1, 0, 1]),
        'R' => Some([1, 0, 0, 0, 0, 1, 1, 0, 0]),
        'S' => Some([0, 0, 1, 0, 0, 1, 1, 0, 0]),
        'T' => Some([0, 0, 0, 0, 1, 1, 1, 0, 0]),
        'U' => Some([1, 1, 0, 0, 0, 0, 0, 0, 1]),
        'V' => Some([0, 1, 1, 0, 0, 0, 0, 0, 1]),
        'W' => Some([1, 1, 1, 0, 0, 0, 0, 0, 0]),
        'X' => Some([0, 1, 0, 0, 1, 0, 0, 0, 1]),
        'Y' => Some([1, 1, 0, 0, 1, 0, 0, 0, 0]),
        'Z' => Some([0, 1, 1, 0, 1, 0, 0, 0, 0]),
        '-' => Some([0, 1, 0, 0, 0, 0, 1, 0, 1]),
        '.' => Some([1, 1, 0, 0, 0, 0, 1, 0, 0]),
        ' ' => Some([0, 1, 1, 0, 0, 0, 1, 0, 0]),
        '*' => Some([0, 1, 0, 0, 1, 0, 1, 0, 0]), // Start/stop delimiter
        '$' => Some([0, 1, 0, 1, 0, 1, 0, 0, 0]),
        '/' => Some([0, 1, 0, 1, 0, 0, 0, 1, 0]),
        '+' => Some([0, 1, 0, 0, 0, 1, 0, 1, 0]),
        '%' => Some([0, 0, 0, 1, 0, 1, 0, 1, 0]),
        _ => None,
    }
}

pub fn generate_code39(text: &str) -> BarPattern {
    let mut modules = Vec::new();
    let content = format!("*{}*", text.trim_matches('*'));

    // Leading quiet zone
    for _ in 0..10 {
        modules.push(false);
    }

    for (char_idx, ch) in content.chars().enumerate() {
        if char_idx > 0 {
            // Inter-character narrow space
            modules.push(false);
        }
        if let Some(pattern) = code39_char_pattern(ch) {
            for (idx, &is_wide) in pattern.iter().enumerate() {
                let is_bar = idx % 2 == 0;
                let count = if is_wide == 1 { 3 } else { 1 };
                for _ in 0..count {
                    modules.push(is_bar);
                }
            }
        } else {
            // Fallback for unencodable char
            for _ in 0..9 {
                modules.push(false);
            }
        }
    }

    // Trailing quiet zone
    for _ in 0..10 {
        modules.push(false);
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

    // Leading quiet zone (10 modules)
    for _ in 0..10 {
        modules.push(false);
    }

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

    // Trailing quiet zone
    for _ in 0..10 {
        modules.push(false);
    }

    BarPattern { modules }
}
