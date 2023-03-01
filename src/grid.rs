use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui::{
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::cell::{Cell, CellValue};

#[derive(Clone, Debug)]
pub struct Grid {
    width: usize,
    height: usize,

    lids: char,
    sides: char,
    corners: Option<[char; 4]>,

    cursor: (usize, usize),
    last_move: Instant,

    inner: Vec<Vec<Cell>>,
}

impl Widget for Grid {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let width = std::cmp::min(2 * self.width, area.width as usize - 2) as u32;
        let height = std::cmp::min(self.height + 1, area.height as usize - 2) as u16;

        let lid = self.lids.to_string().repeat(width as usize + 1);

        let top_lid = format!(
            "{}{lid}{}",
            self.corners.map(|arr| arr[0]).unwrap_or(' '),
            self.corners.map(|arr| arr[1]).unwrap_or(' ')
        );

        let bot_lid = format!(
            "{}{lid}{}",
            self.corners.map(|arr| arr[2]).unwrap_or(' '),
            self.corners.map(|arr| arr[3]).unwrap_or(' ')
        );

        buf.set_string(area.left(), area.top(), top_lid.as_str(), Style::default());

        self.inner
            .iter()
            .map(|line| {
                line.iter()
                    .map(|c| char::from(c.value).to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            })
            .enumerate()
            .for_each(|(index, line)| {
                buf.set_string(
                    area.left(),
                    area.top() + index as u16 + 1,
                    format!("{1} {0} {1}", line, self.sides),
                    Style::default(),
                )
            });

        buf.set_string(
            area.left(),
            area.top() + height,
            bot_lid.as_str(),
            Style::default(),
        );

        let (x, y) = self.cursor;
        let (x, y) = (area.left() + 2 + 2 * x as u16, area.top() + 1 + y as u16);
        let val = buf.get(x, y).symbol.clone();
        let blink = self.last_move.elapsed() < Duration::from_millis(500)
            || SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                % 2
                == 0;

        buf.set_string(
            x,
            y,
            val,
            Style::default()
                .fg(if blink { Color::Black } else { Color::Cyan })
                .bg(if blink { Color::Cyan } else { Color::Black })
                .add_modifier(Modifier::SLOW_BLINK | Modifier::BOLD),
        );
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new(10, 10)
    }
}

impl From<String> for Grid {
    fn from(value: String) -> Self {
        let mut res = Grid::empty();

        value.lines().for_each(|line| res.add_line(Some(line)));

        res
    }
}

impl Grid {
    fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            inner: vec![],
            ..Default::default()
        }
    }

    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            lids: '─',
            sides: '│',
            corners: Some(['╭', '╮', '╰', '╯']),
            cursor: Default::default(),
            inner: vec![vec![CellValue::Empty.into(); width]; height],
            last_move: Instant::now(),
        }
    }

    /// Adds a new column.
    /// Resizes grid.
    pub fn add_column(&mut self) {
        self.width += 1;

        self.inner
            .iter_mut()
            .for_each(|row| row.push(CellValue::Empty.into()));
    }

    /// Adds a new line, either blank or filled with desired string.
    /// Resizes grid as necessary.
    pub fn add_line(&mut self, line: Option<&str>) {
        self.height += 1;

        if let Some(line) = line {
            let mut line = line.chars().map(Cell::from).collect::<Vec<Cell>>();

            // If longer than width, resize all other rows to keep rectangular shape
            if line.len() > self.width {
                let size = line.len();
                self.width = size;
                self.inner
                    .iter_mut()
                    .for_each(|row| row.resize(size, CellValue::Empty.into()));
            } else {
                line.resize(self.width, CellValue::Empty.into());
            }

            self.inner.push(line);
        } else {
            self.inner.push(vec![CellValue::Empty.into(); self.width]);
        }
    }

    /// Moves cursor by an offset, possibly extending the grid to the right
    pub fn move_cursor(&mut self, x: i32, y: i32) -> Result<(), (i32, i32)> {
        let (og_x, og_y) = self.cursor;
        let (new_x, new_y) = (og_x as i32 + x, og_y as i32 + y);

        if new_x >= 0 && new_y >= 0 {
            if new_x as usize >= self.width {
                self.add_column();
            } else if new_y as usize >= self.height {
                self.add_line(None);
            }

            return self
                .set_cursor(new_x as usize, new_y as usize)
                .map_err(|(x, y)| (x as i32, y as i32));
        }

        Err((new_x, new_y))
    }

    /// Sets current cursor position
    pub fn set_cursor(&mut self, x: usize, y: usize) -> Result<(), (usize, usize)> {
        self.last_move = Instant::now();

        if !(0..(self.width.into())).contains(&x) || !(0..(self.height.into())).contains(&y) {
            return Err((x, y));
        }

        self.cursor = (x, y);

        Ok(())
    }

    /// Gets current cursor position
    pub fn get_cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// Returns size tuple
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Completely clears grid
    pub fn clear(&mut self) {
        self.inner = vec![vec![CellValue::Empty.into(); self.width]; self.height];
    }

    /// Set characters for lids and walls
    pub fn style(mut self, lids: char, sides: char) -> Self {
        self.lids = lids;
        self.sides = sides;
        self
    }

    #[inline]
    /// Get cell value at position
    pub fn get(&self, x: usize, y: usize) -> Cell {
        self.inner.get(y).unwrap()[x]
    }

    #[inline]
    /// Set cell at position to desired value
    pub fn set(&mut self, x: usize, y: usize, val: CellValue) {
        self.inner.get_mut(y).unwrap()[x].value = val;
    }

    /// Set cell under cursor to desired value
    pub fn set_current(&mut self, val: CellValue) {
        let (x, y) = self.cursor;
        self.set(x, y, val);
    }
}
