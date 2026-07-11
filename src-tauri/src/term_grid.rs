use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CellDiff {
    pub row: usize,
    pub col: usize,
    pub ch: char,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TermGrid {
    cols: usize,
    rows: usize,
    cells: Vec<char>,
    previous: Vec<char>,
    alt_screen: bool,
}

impl TermGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = vec![' '; cols * rows];
        Self { cols, rows, previous: cells.clone(), cells, alt_screen: false }
    }

    pub fn feed_utf8_lossy(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        if text.contains("\x1b[?1049h") {
            self.alt_screen = true;
            self.cells.fill(' ');
        }
        if text.contains("\x1b[?1049l") {
            self.alt_screen = false;
            self.cells.fill(' ');
        }
        let mut row = 0;
        let mut col = 0;
        let mut escape = false;
        for ch in text.chars() {
            if escape {
                if ch.is_ascii_alphabetic() {
                    escape = false;
                }
                continue;
            }
            match ch {
                '\x1b' => escape = true,
                '\n' => {
                    row = (row + 1).min(self.rows.saturating_sub(1));
                    col = 0;
                }
                '\r' => col = 0,
                ch if !ch.is_control() && row < self.rows && col < self.cols => {
                    self.cells[row * self.cols + col] = ch;
                    col += 1;
                }
                _ => {}
            }
        }
    }

    pub fn diff(&mut self) -> Vec<CellDiff> {
        let mut out = Vec::new();
        for (idx, ch) in self.cells.iter().enumerate() {
            if self.alt_screen || self.previous[idx] != *ch {
                out.push(CellDiff {
                    row: idx / self.cols,
                    col: idx % self.cols,
                    ch: *ch,
                    fg: None,
                    bg: None,
                    flags: Vec::new(),
                });
            }
        }
        self.previous.clone_from(&self.cells);
        out
    }

    pub fn is_alt_screen(&self) -> bool {
        self.alt_screen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_simple_grid() {
        let mut grid = TermGrid::new(10, 2);
        grid.feed_utf8_lossy(b"hi\nok");
        insta::assert_yaml_snapshot!(grid.diff());
    }

    #[test]
    fn detects_alt_screen() {
        let mut grid = TermGrid::new(10, 2);
        grid.feed_utf8_lossy(b"\x1b[?1049h");
        assert!(grid.is_alt_screen());
        grid.feed_utf8_lossy(b"\x1b[?1049l");
        assert!(!grid.is_alt_screen());
    }
}
