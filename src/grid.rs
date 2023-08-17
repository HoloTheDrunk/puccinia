use tui::{layout::Rect, widgets::StatefulWidget};

use crate::{
    cell::{Cell, CellValue, Direction},
    frontend::{self, EditorMode},
};

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

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

    pan: (usize, usize),

    inner: VecDeque<VecDeque<Cell>>,
}

impl StatefulWidget for Grid {
    type State = frontend::State;

    fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
        // let width = std::cmp::min(2 * self.width, area.width as usize - 2) as u32;
        let height = std::cmp::min(self.height + 1, area.height as usize - 2) as u16;

        let default_style = Style::default().fg(Color::White).bg(Color::Reset);

        let target_cell_count = (area.width as usize / 2 - 2 - self.pan.0).min(self.inner[0].len());
        let clip_right = ((target_cell_count - self.pan.0) * 2 + 1) > area.width as usize;

        let lid_length = (target_cell_count - self.pan.0) * 2 + 1 + (self.pan.0 != 0) as usize;
        let lid = self.lids.to_string().repeat(lid_length);
        let (mut top_lid, mut bot_lid) = (String::new(), String::new());

        if self.pan.1 == 0 {
            if self.pan.0 == 0 {
                top_lid.push(self.corners.map(|arr| arr[0]).unwrap_or(' '));
            }

            top_lid.push_str(lid.as_ref());

            if !clip_right {
                top_lid.push(self.corners.map(|arr| arr[1]).unwrap_or(' '));
            }
        } else {
            top_lid = format!(
                "{}{}{}",
                if self.pan.0 == 0 { self.sides } else { ' ' },
                " ".repeat(lid_length - (self.pan.0 != 0) as usize).as_str(),
                self.sides
            );
        }

        if (self.height - self.pan.1) < area.height as usize {
            if self.pan.0 == 0 {
                bot_lid.push(self.corners.map(|arr| arr[2]).unwrap_or(' '));
            }

            bot_lid.push_str(lid.as_ref());

            if !clip_right {
                bot_lid.push(self.corners.map(|arr| arr[3]).unwrap_or(' '));
            }
        }

        buf.set_string(area.left(), area.top(), top_lid.as_str(), default_style);

        let left_side = Span::styled(
            format!("{} ", if self.pan.0 == 0 { self.sides } else { ' ' }),
            default_style,
        );
        let right_side = Span::styled(
            format!(" {}", if !clip_right { self.sides } else { ' ' }),
            default_style,
        );

        self.inner
            .iter()
            .skip(self.pan.1)
            .take(area.height as usize - 2)
            .map(|line| {
                let mut spans = intersperse(
                    line.iter()
                        .skip(self.pan.0)
                        .take(target_cell_count)
                        .map(|cell| cell.to_span(&state.config)),
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

        if (self.height - self.pan.1) < area.height as usize {
            buf.set_string(
                area.left(),
                area.top() + height - self.pan.1 as u16,
                bot_lid.as_str(),
                default_style,
            );
        }

        if let EditorMode::Visual(start, end) = state.mode {
            let (start, end) = (
                (
                    area.left() + 2 + 2 * start.0.min(end.0) as u16,
                    area.top() + 1 + start.1.min(end.1) as u16,
                ),
                (
                    area.left() + 2 + 2 * end.0.max(start.0) as u16,
                    area.top() + 1 + end.1.max(start.1) as u16,
                ),
            );

            buf.set_style(
                Rect::new(start.0, start.1, end.0 - start.0 + 1, end.1 - start.1 + 1),
                Style::default().bg(Color::Cyan),
            );
        }

        let (x, y) = self.cursor;
        let (x, y) = (area.left() + 2 + 2 * x as u16, area.top() + 1 + y as u16);
        let blink = self.last_move.elapsed() < Duration::from_millis(1000)
            || Instant::now().duration_since(self.last_move).as_secs() % 2 == 0;

        let cursor_color = Color::from(&state.mode);
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

        res.load_values(value);

        res
    }
}

impl Grid {
    fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            inner: VecDeque::new(),
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
            last_move: Instant::now(),

            inner: vec![vec![CellValue::Empty.into(); width].into(); height].into(),

            pan: (0, 0),
        }
    }

    pub fn load_values(&mut self, grid: String) {
        self.clear_values();

        if grid.is_empty() {
            self.append_line(Some(" "));
        } else {
            grid.lines().for_each(|line| self.append_line(Some(line)));
            // self.width = grid.lines().map(|line| line.len()).max().unwrap();
            // self.height = grid.lines().count();
        }

        self.trim();
    }

    pub fn load_breakpoints(&mut self, breakpoints: Vec<(usize, usize)>) {
        self.clear_breakpoints();
        breakpoints
            .into_iter()
            .for_each(|(x, y)| self.toggle_breakpoint(x, y));
    }

    /// Adds a new column to the left side of the grid.
    /// Resizes grid.
    pub fn prepend_column(&mut self) {
        self.width += 1;

        self.inner
            .iter_mut()
            .for_each(|row| row.push_front(CellValue::Empty.into()));
    }

    /// Adds a new column to the right side of the grid.
    /// Resizes grid.
    pub fn append_column(&mut self) {
        self.width += 1;

        self.inner
            .iter_mut()
            .for_each(|row| row.push_back(CellValue::Empty.into()));
    }

    /// Adds a new line to the top of the grid, either blank or filled with desired string.
    /// Resizes grid as necessary.
    pub fn prepend_line(&mut self, line: Option<&str>) {
        self.height += 1;

        if let Some(line) = line {
            let mut line = line.chars().map(Cell::from).collect::<VecDeque<Cell>>();

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

            self.inner.push_front(line);
        } else {
            self.inner
                .push_front(vec![CellValue::Empty.into(); self.width].into());
        }
    }

    pub fn trim(&mut self) -> [usize; 4] {
        let lead_col: usize = self
            .inner
            .iter()
            .map(|line| {
                line.iter()
                    .take_while(|c| c.value == CellValue::Empty)
                    .count()
            })
            .min()
            .unwrap_or(0);

        let trail_col: usize = self
            .inner
            .iter()
            .map(|line| {
                line.iter()
                    .rev()
                    .take_while(|c| c.value == CellValue::Empty)
                    .count()
            })
            .min()
            .unwrap_or(0);

        let lead_row: usize = self
            .inner
            .iter()
            .take_while(|line| line.iter().all(|cell| cell.value == CellValue::Empty))
            .count();

        let trail_row: usize = self
            .inner
            .iter()
            .rev()
            .take_while(|line| line.iter().all(|cell| cell.value == CellValue::Empty))
            .count();

        (0..lead_row).for_each(|_| {
            self.inner.pop_front();
        });
        (0..trail_row).for_each(|_| {
            self.inner.pop_back();
        });

        self.height -= (lead_row + trail_row).min(self.height);

        self.inner.iter_mut().for_each(|line| {
            (0..lead_col).for_each(|_| {
                line.pop_front();
            });
            (0..trail_col).for_each(|_| {
                line.pop_back();
            });
        });

        self.width -= (lead_col + trail_col).min(self.width);

        if self.width == 0 {
            self.inner
                .push_front(vec![CellValue::Empty.into(); 1].into());
        }

        [lead_row, trail_row, lead_col, trail_col]
    }

    /// Adds a new line to the bottom of the grid, either blank or filled with desired string.
    /// Resizes grid as necessary.
    pub fn append_line(&mut self, line: Option<&str>) {
        self.height += 1;

        if let Some(line) = line {
            let mut line = line.chars().map(Cell::from).collect::<VecDeque<Cell>>();

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

            self.inner.push_back(line);
        } else {
            self.inner
                .push_back(vec![CellValue::Empty.into(); self.width].into());
        }
    }

    /// Moves cursor by an offset, possibly extending the grid to the right. Returns whether or not
    /// the cursor was wrapped around the grid.
    pub fn move_cursor(&mut self, dir: Direction, update_dir: bool, resize: bool) -> bool {
        if update_dir {
            self.cursor_direction = dir;
        }

        let (x, y) = dir.into();
        let (og_x, og_y) = self.cursor;
        let (mut new_x, mut new_y) = (og_x as i32 + x, og_y as i32 + y);

        let wrapped = if resize {
            if new_x < 0 {
                self.prepend_column();
                new_x = 0;
            } else if new_x == self.width as i32 {
                self.append_column();
            }

            if new_y < 0 {
                self.prepend_line(None);
                new_y = 0;
            } else if new_y == self.height as i32 {
                self.append_line(None);
            }

            false
        } else {
            let wrap = |val: i32, max: i32| {
                if val < 0 {
                    (true, max - 1)
                } else if val >= max {
                    (true, 0)
                } else {
                    (false, val)
                }
            };
            let (is_wrapped_x, wrapped_x) = wrap(new_x, self.width as i32);
            let (is_wrapped_y, wrapped_y) = wrap(new_y, self.height as i32);
            new_x = wrapped_x;
            new_y = wrapped_y;

            is_wrapped_x | is_wrapped_y
        };

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

    pub fn pan(&mut self, dir: Direction) {
        match dir {
            Direction::Up => self.pan = (self.pan.0, self.pan.1.saturating_sub(1)),
            Direction::Down => self.pan = (self.pan.0, (self.pan.1 + 1).min(self.height - 1)),
            Direction::Left => self.pan = (self.pan.0.saturating_sub(1), self.pan.1),
            Direction::Right => self.pan = ((self.pan.0 + 1).min(self.width - 1), self.pan.1),
            Direction::Random => unreachable!(),
        }
    }

    /// Loops over an area, running the provided function.
    pub fn loop_over<F>(&mut self, (start, end): ((usize, usize), (usize, usize)), mut func: F)
    where
        F: FnMut(usize, usize, &mut Cell),
    {
        for x in (start.0.min(end.0))..=(end.0.max(start.0)) {
            for y in (start.1.min(end.1))..=(end.1.max(start.1)) {
                func(x, y, self.inner.get_mut(y).unwrap().get_mut(x).unwrap());
            }
        }
    }

    /// Completely clears grid
    pub fn clear(&mut self) {
        self.inner = vec![vec![CellValue::Empty.into(); self.width].into(); self.height].into();
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

    pub fn check_bounds(&self, (x, y): (usize, usize)) -> bool {
        x < self.width && y < self.height
    }
}
