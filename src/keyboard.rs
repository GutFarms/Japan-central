//! Compact on-screen keyboard for CYD 320×240 setup screens.

/// Screen point in landscape pixels (0..319, 0..239).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Char(char),
    Backspace,
    Shift,
    Symbols,
    Space,
    Enter,
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyLayer {
    Lower,
    Upper,
    Sym,
}

impl KeyLayer {
    pub fn toggle_shift(self) -> Self {
        match self {
            KeyLayer::Lower => KeyLayer::Upper,
            KeyLayer::Upper => KeyLayer::Lower,
            KeyLayer::Sym => KeyLayer::Sym,
        }
    }

    pub fn toggle_sym(self) -> Self {
        match self {
            KeyLayer::Sym => KeyLayer::Lower,
            _ => KeyLayer::Sym,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Keyboard {
    pub layer: KeyLayer,
    /// Top of keyboard in screen pixels.
    pub origin_y: i32,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            layer: KeyLayer::Lower,
            origin_y: 96,
        }
    }
}

/// One drawn key: label + bounds.
#[derive(Clone, Copy, Debug)]
pub struct KeyHit {
    pub action: KeyAction,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub label: &'static str,
}

const KEY_H: u32 = 26;
const KEY_GAP: i32 = 2;
const ROW_H: i32 = 28;

impl Keyboard {
    pub fn rows(&self) -> [&'static str; 4] {
        match self.layer {
            KeyLayer::Lower => ["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm"],
            KeyLayer::Upper => ["1234567890", "QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"],
            KeyLayer::Sym => ["1234567890", "-_.:/@+*=", "#$%&!?,'\"", "()[]{}"],
        }
    }

    /// Iterate all hittable keys for the current layer.
    pub fn keys(&self) -> heapless::Vec<KeyHit, 64> {
        let mut out: heapless::Vec<KeyHit, 64> = heapless::Vec::new();
        let y0 = self.origin_y;

        for (ri, row) in self.rows().iter().enumerate() {
            let n = row.len().max(1) as i32;
            let total_gap = KEY_GAP * (n - 1);
            let key_w = ((320 - 8 - total_gap) / n).max(18) as u32;
            let row_width = n * key_w as i32 + total_gap;
            let x0 = (320 - row_width) / 2;
            let y = y0 + ri as i32 * ROW_H;
            for (ci, ch) in row.chars().enumerate() {
                let x = x0 + ci as i32 * (key_w as i32 + KEY_GAP);
                let _ = out.push(KeyHit {
                    action: KeyAction::Char(ch),
                    x,
                    y,
                    w: key_w,
                    h: KEY_H,
                    label: char_to_static(ch),
                });
            }
        }

        let y = y0 + 4 * ROW_H;
        let shift_lbl = if self.layer == KeyLayer::Upper {
            "ABC"
        } else {
            "sh"
        };
        let sym_lbl = if self.layer == KeyLayer::Sym {
            "abc"
        } else {
            "?#"
        };
        let actions: [(KeyAction, &'static str, i32, u32); 5] = [
            (KeyAction::Shift, shift_lbl, 4, 44),
            (KeyAction::Symbols, sym_lbl, 52, 40),
            (KeyAction::Space, "space", 96, 100),
            (KeyAction::Backspace, "<x", 200, 44),
            (KeyAction::Enter, "OK", 248, 68),
        ];
        for (action, label, x, w) in actions {
            let _ = out.push(KeyHit {
                action,
                x,
                y,
                w,
                h: KEY_H + 2,
                label,
            });
        }

        let _ = out.push(KeyHit {
            action: KeyAction::Skip,
            x: 260,
            y: y0 - 18,
            w: 54,
            h: 16,
            label: "skip",
        });

        out
    }

    pub fn hit_test(&self, p: TouchPoint) -> Option<KeyAction> {
        let x = p.x as i32;
        let y = p.y as i32;
        for key in self.keys() {
            if x >= key.x
                && x < key.x + key.w as i32
                && y >= key.y
                && y < key.y + key.h as i32
            {
                return Some(key.action);
            }
        }
        None
    }

    /// Apply key. Returns `true` when the field is complete (Enter / Skip).
    pub fn apply(&mut self, action: KeyAction, buf: &mut heapless::String<128>) -> bool {
        match action {
            KeyAction::Char(c) => {
                let _ = buf.push(c);
                false
            }
            KeyAction::Backspace => {
                let _ = buf.pop();
                false
            }
            KeyAction::Space => {
                let _ = buf.push(' ');
                false
            }
            KeyAction::Shift => {
                self.layer = self.layer.toggle_shift();
                false
            }
            KeyAction::Symbols => {
                self.layer = self.layer.toggle_sym();
                false
            }
            KeyAction::Enter => true,
            KeyAction::Skip => {
                buf.clear();
                let _ = buf.push_str("-");
                true
            }
        }
    }
}

fn char_to_static(c: char) -> &'static str {
    match c {
        '0' => "0", '1' => "1", '2' => "2", '3' => "3", '4' => "4",
        '5' => "5", '6' => "6", '7' => "7", '8' => "8", '9' => "9",
        'a' => "a", 'b' => "b", 'c' => "c", 'd' => "d", 'e' => "e",
        'f' => "f", 'g' => "g", 'h' => "h", 'i' => "i", 'j' => "j",
        'k' => "k", 'l' => "l", 'm' => "m", 'n' => "n", 'o' => "o",
        'p' => "p", 'q' => "q", 'r' => "r", 's' => "s", 't' => "t",
        'u' => "u", 'v' => "v", 'w' => "w", 'x' => "x", 'y' => "y",
        'z' => "z",
        'A' => "A", 'B' => "B", 'C' => "C", 'D' => "D", 'E' => "E",
        'F' => "F", 'G' => "G", 'H' => "H", 'I' => "I", 'J' => "J",
        'K' => "K", 'L' => "L", 'M' => "M", 'N' => "N", 'O' => "O",
        'P' => "P", 'Q' => "Q", 'R' => "R", 'S' => "S", 'T' => "T",
        'U' => "U", 'V' => "V", 'W' => "W", 'X' => "X", 'Y' => "Y",
        'Z' => "Z",
        '-' => "-", '_' => "_", '.' => ".", ':' => ":", '/' => "/",
        '@' => "@", '+' => "+", '*' => "*", '=' => "=",
        '#' => "#", '$' => "$", '%' => "%", '&' => "&", '!' => "!",
        '?' => "?", ',' => ",", '\'' => "'", '"' => "\"",
        '(' => "(", ')' => ")", '[' => "[", ']' => "]", '{' => "{", '}' => "}",
        _ => "?",
    }
}

/// Hit-test for main GUI chrome (tabs / menu rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuiHit {
    Tab(usize),
    MenuRow(usize),
    ChangeBanner,
}

pub fn hit_gui(p: TouchPoint, on_menu: bool) -> Option<GuiHit> {
    let x = p.x as i32;
    let y = p.y as i32;
    if (28..54).contains(&y) {
        let tab = (x.clamp(0, 319) / 80) as usize;
        return Some(GuiHit::Tab(tab.min(3)));
    }
    if on_menu {
        if (86..116).contains(&y) {
            return Some(GuiHit::MenuRow(0));
        }
        if (122..152).contains(&y) {
            return Some(GuiHit::MenuRow(1));
        }
    }
    if !on_menu && (60..200).contains(&y) && (8..312).contains(&x) {
        return Some(GuiHit::ChangeBanner);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_skip_complete() {
        let mut kb = Keyboard::default();
        let mut s: heapless::String<128> = heapless::String::new();
        assert!(!kb.apply(KeyAction::Char('a'), &mut s));
        assert_eq!(s.as_str(), "a");
        assert!(kb.apply(KeyAction::Enter, &mut s));
        s.clear();
        assert!(kb.apply(KeyAction::Skip, &mut s));
        assert_eq!(s.as_str(), "-");
    }

    #[test]
    fn hit_bottom_enter() {
        let kb = Keyboard::default();
        let p = TouchPoint {
            x: 280,
            y: (kb.origin_y + 4 * 28 + 10) as u16,
        };
        assert_eq!(kb.hit_test(p), Some(KeyAction::Enter));
    }

    #[test]
    fn tab_hit() {
        assert_eq!(
            hit_gui(TouchPoint { x: 10, y: 40 }, false),
            Some(GuiHit::Tab(0))
        );
        assert_eq!(
            hit_gui(TouchPoint { x: 200, y: 40 }, false),
            Some(GuiHit::Tab(2))
        );
    }
}
