use tui::{layout::Rect, widgets::StatefulWidget};

use crate::{
    cell::{Cell, CellValue, Direction},
    frontend::{self, EditorMode},
};

use std::time::{Duration, Instant};

use {
    itertools::intersperse,
    tui::{
        style::{Color, Modifier, Style},
        text::{Span, Spans},
    },
};

#[derive(Clone, Debug)]
pub struct Grid {
    width: usize,
    height: usize,

    pub lids: char,
    pub sides: char,
    pub corners: Option<[char; 4]>,

    cursor: (usize, usize),
    cursor_direction: Direction,
    last_move: Instant,

    inner: Vec<Vec<Cell>>,
}

impl StatefulWidget for Grid {
    type State = (EditorMode, frontend::Config);

    fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
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

        let default_style = Style::default().fg(Color::White).bg(Color::Reset);
        buf.set_string(area.left(), area.top(), top_lid.as_str(), default_style);

        let left_side = Span::styled(format!("{} ", self.sides), default_style);
        let right_side = Span::styled(format!(" {}", self.sides), default_style);

        self.inner
            .iter()
            .map(|line| {
                let mut spans = intersperse(
                    line.iter().map(|cell| cell.to_span(&state.1)),
                    Span::styled(" ", default_style),
                )
                .collect::<Vec<_>>();

                let mut line = vec![left_side.clone()];
                line.append(&mut spans);
                line.push(right_side.clone());

                Spans::from(line)
            })
            .enumerate()
            .for_each(|(index, line)| {
                buf.set_spans(
                    area.left(),
                    area.top() + index as u16 + 1,
                    &line,
                    line.0.len() as u16 + 2,
                );
            });

        buf.set_string(
            area.left(),
            area.top() + height,
            bot_lid.as_str(),
            default_style,
        );

        let (x, y) = self.cursor;
        let (x, y) = (area.left() + 2 + 2 * x as u16, area.top() + 1 + y as u16);
        let blink = self.last_move.elapsed() < Duration::from_millis(1000)
            || Instant::now().duration_since(self.last_move).as_secs() % 2 == 0;

        let cursor_color = match state.0 {
            EditorMode::Normal => Color::White,
            EditorMode::Command(_) => Color::DarkGray,
            EditorMode::Insert => Color::Yellow,
            EditorMode::Running => Color::Red,
        };
        let cursor_style = if blink {
            Style::default().bg(cursor_color)
        } else {
            Style::default().fg(cursor_color)
        };

        buf.set_style(
            Rect::new(x, y, 1, 1),
            cursor_style.add_modifier(Modifier::SLOW_BLINK | Modifier::BOLD),
        );

        // BreakPoint
        let bp_positions = self.get_breakpoints();

        for (x, y) in bp_positions {
            let target = Rect {
                x: area.left() + 2 + x as u16 * 2,
                y: area.top() + 1 + y as u16,
                width: 1,
                height: 1,
            };

            buf.set_style(target, Style::default().bg(Color::Rgb(64, 64, 64)));
        }
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

        res.load(value);

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
            cursor_direction: Direction::Right,
            inner: vec![vec![CellValue::Empty.into(); width]; height],
            last_move: Instant::now(),
        }
    }

    pub fn load(&mut self, grid: String) {
        self.clear();
        if grid.is_empty() {
            self.add_line(Some(" "))
        } else {
            grid.lines().for_each(|line| self.add_line(Some(line)));
        }
    }

    pub fn load_breakpoints(&mut self, breakpoints: Vec<(usize, usize)>) {
        self.clear_breakpoints();
        breakpoints
            .into_iter()
            .for_each(|(x, y)| self.toggle_breakpoint(x, y));
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

    /// Moves cursor by an offset, possibly extending the grid to the right. Returns whether or not
    /// the cursor was wrapped around the grid.
    pub fn move_cursor(&mut self, dir: Direction, update_dir: bool) -> bool {
        if update_dir {
            self.cursor_direction = dir;
        }

        let (x, y) = dir.into();
        let (og_x, og_y) = self.cursor;
        let (new_x, new_y) = (og_x as i32 + x, og_y as i32 + y);

        let wrap = |val: i32, max: i32| {
            if val < 0 {
                (true, max - 1)
            } else if val >= max {
                (true, 0)
            } else {
                (false, val)
            }
        };
        let (wrapped_x, new_x) = wrap(new_x, self.width as i32);
        let (wrapped_y, new_y) = wrap(new_y, self.height as i32);
        let wrapped = wrapped_x | wrapped_y;

        self.set_cursor(new_x as usize, new_y as usize).expect(
            "Invalid move; this should be impossible, please contact the developer through a GitHub issue.",
        );

        wrapped
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

    pub fn get_cursor_dir(&self) -> Direction {
        self.cursor_direction
    }

    pub fn set_cursor_dir(&mut self, dir: Direction) {
        self.cursor_direction = dir;
    }

    /// Returns size tuple
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Completely clears grid
    pub fn clear(&mut self) {
        self.inner = vec![vec![CellValue::Empty.into(); self.width]; self.height];
    }

    /// Clears all cell values, keeping breakpoint and heat information
    pub fn clear_values(&mut self) {
        for line in &mut self.inner {
            for cell in line {
                cell.value = CellValue::Empty;
            }
        }
    }

    #[inline]
    /// Get cell value at position
    pub fn get(&self, x: usize, y: usize) -> Cell {
        self.inner.get(y).unwrap()[x]
    }

    /// Get cell value at current position
    pub fn get_current(&self) -> Cell {
        let (x, y) = self.cursor;
        self.get(x, y)
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

    pub fn get_breakpoints(&self) -> Vec<(usize, usize)> {
        self.inner
            .iter()
            .enumerate()
            .flat_map(|(y, line)| {
                line.iter()
                    .enumerate()
                    .flat_map(|(x, cell)| cell.is_breakpoint.then_some((x, y)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }

    #[inline]
    /// Toggle breakpoint at position
    pub fn toggle_breakpoint(&mut self, x: usize, y: usize) {
        self.inner.get_mut(y).unwrap()[x].is_breakpoint = !self.get(x, y).is_breakpoint;
    }

    /// Toggle breakpoint under cursor
    pub fn toggle_current_breakpoint(&mut self) {
        let (x, y) = self.cursor;
        self.toggle_breakpoint(x, y);
    }

    pub fn clear_breakpoints(&mut self) {
        for line in &mut self.inner {
            for cell in line {
                cell.is_breakpoint = false;
            }
        }
    }

    #[inline]
    /// Set cell heat at position to desire value
    pub fn set_heat(&mut self, x: usize, y: usize, heat: u8) {
        self.inner.get_mut(y).unwrap()[x].heat = heat;
    }

    /// Set cell heat under cursor to desired value
    pub fn set_current_heat(&mut self, heat: u8) {
        let (x, y) = self.cursor;
        self.set_heat(x, y, heat);
    }

    pub fn reduce_heat(&mut self, amount: u8) {
        for line in &mut self.inner {
            for cell in line {
                cell.heat = cell.heat.saturating_sub(amount);
            }
        }
    }

    pub fn clear_heat(&mut self) {
        for line in &mut self.inner {
            for cell in line {
                cell.heat = 0;
            }
        }
    }

    /// Dump grid contents as a string.
    pub fn dump(&self) -> String {
        let mut res = String::new();

        let cells = self
            .inner
            .iter()
            .map(|v| v.iter().map(|cell| cell.value).collect::<Vec<_>>())
            .collect::<Vec<_>>();

        for line in cells {
            for cell in line {
                res.push(cell.into());
            }
            res.push('\n');
        }

        res
    }
}
