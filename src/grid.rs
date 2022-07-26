use bevy::prelude::*;

use std::ops::{Add, Sub};

#[derive(Default)]
pub struct Grid {
    pub grid: Vec<Vec<Cell>>,
}

/// Represents a single cell of the grid.
pub struct Cell {
    /// The content of the cell
    pub value: CellValue,
    /// Heat represents how long ago the cell was last "visited" by a cursor.
    pub heat: i8,
}

pub enum CellValue {
    Empty,
    Op(Operator),
    Dir(Direction),
    If(IfDir),
    StringMode,
    Bridge,
    End,
    Number(i32),
    Unrecognized,
}

pub enum Operator {
    Nullary(Input),
    Unary(UnaryOperator),
    Binary(BinaryOperator),
    Ternary(TernaryOperator),
}

pub enum Input {
    Integer,
    Ascii,
}

pub enum UnaryOperator {
    Negate,
    Duplicate,
    Pop,
    WriteNumber,
    WriteASCII,
}

pub enum BinaryOperator {
    Greater,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Swap,
    Get,
}

pub enum TernaryOperator {
    Put,
}

pub enum IfDir {
    Horizontal,
    Vertical,
}

#[derive(Default)]
struct Limit<T: Add + Sub + PartialEq + PartialOrd + Copy> {
    min: T,
    max: T,
}

impl<T: Add<Output = T> + Sub<Output = T> + PartialOrd + Copy> Limit<T> {
    pub fn new(min: T, max: T) -> Self {
        Limit { min, max }
    }

    fn wrap(&self, val: T) -> T {
        if val > self.max {
            return self.min + (self.max - val);
        }

        if val < self.min {
            return self.max - (self.min - val);
        }

        val
    }
}

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Random,
}

#[derive(Component, Default)]
pub struct Cursor {
    limits: (Limit<i32>, Limit<i32>),
    pub x: i32,
    pub y: i32,
    pub stack: Vec<i32>,
}

impl Cursor {
    fn shift(&mut self, x: i32, y: i32) {
        self.x = self.limits.0.wrap(self.x + x);
        self.y = self.limits.1.wrap(self.y + y);
    }

    pub fn r#move(&mut self, dir: Direction) {
        match dir {
            Direction::Up => self.shift(0, 1),
            Direction::Down => self.shift(0, -1),
            Direction::Left => self.shift(-1, 0),
            Direction::Right => self.shift(1, 0),
            Direction::Random => {
                let dir = rand::random::<bool>() as i32 * 2 - 1;

                if rand::random() {
                    self.shift(dir, 0);
                } else {
                    self.shift(0, dir);
                }
            }
        }
    }
}

fn setup(mut commands: Commands, mut grid: ResMut<Grid>) {
    commands.spawn()
        .insert(Cursor {
            limits: (Limit::new(0, 80), Limit::new(0, 25)),
            ..Default::default()
        });

    for _ in 0..80 {
        let mut col = Vec::default();
        for _ in 0..25 {
            col.push(Cell{ value: CellValue::Empty, heat: 0 });
        }
        grid.grid.push(col);
    }
}

pub struct GridPlugin;
impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::default())
            .add_startup_system(setup);
    }
}
