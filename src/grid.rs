// TODO! remove this
#![allow(unused)]

use bevy::prelude::*;
use std::ops::{Add, Sub};

#[derive(Default)]
pub struct Grid {
    pub grid: Vec<Vec<Cell>>,
}

impl Grid {
    fn get(&self, x: usize, y: usize) -> &Cell {
        &self.grid[x][y]
    }

    fn mutate<F>(&mut self, x: usize, y: usize, mut function: F)
    where
        F: FnMut(&mut Cell),
    {
        if let Some(vec) = self.grid.get_mut(x) {
            if let Some(cell) = vec.get_mut(y) {
                function(cell);
                return;
            }
        }

        error!("Could not get value of cell at ({}, {})", x, y);
    }

    fn put(&mut self, x: usize, y: usize, v: CellValue) {
        self.mutate(x, y, |mut cell| cell.value = v);
    }

    fn heat_up(&mut self, x: usize, y: usize) {
        self.mutate(x, y, |mut cell| cell.heat = i8::MAX);
    }
}

/// Represents a single cell of the grid.
#[derive(Clone, Copy)]
pub struct Cell {
    /// The content of the cell
    pub value: CellValue,
    /// Heat represents how long ago the cell was last "visited" by a cursor.
    pub heat: i8,
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub enum Operator {
    Nullary(Input),
    Unary(UnaryOperator),
    Binary(BinaryOperator),
    Ternary(TernaryOperator),
}

#[derive(Clone, Copy)]
pub enum Input {
    Integer,
    Ascii,
}

#[derive(Clone, Copy)]
pub enum UnaryOperator {
    Negate,
    Duplicate,
    Pop,
    WriteNumber,
    WriteASCII,
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub enum TernaryOperator {
    Put,
}

#[derive(Clone, Copy)]
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

#[derive(Default, PartialEq, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    #[default]
    Right,
    Random,
}

#[derive(Component, Default)]
pub struct Cursor {
    limits: (Limit<i32>, Limit<i32>),
    pub x: i32,
    pub y: i32,
    direction: Direction,
    pub stack: Vec<i32>,
}

impl Cursor {
    fn transpose(&mut self, x: i32, y: i32) {
        self.x = self.limits.0.wrap(self.x + x);
        self.y = self.limits.1.wrap(self.y + y);
    }

    pub fn shift(&mut self, dir: Direction) {
        match dir {
            Direction::Up => self.transpose(0, 1),
            Direction::Down => self.transpose(0, -1),
            Direction::Left => self.transpose(-1, 0),
            Direction::Right => self.transpose(1, 0),
            Direction::Random => {
                let new_dir = match (rand::random::<bool>(), rand::random::<bool>()) {
                    (false, false) => Direction::Down,
                    (false, true) => Direction::Up,
                    (true, false) => Direction::Left,
                    (true, true) => Direction::Right,
                };

                self.shift(new_dir);
            }
        }

        if dir != Direction::Random {
            self.direction = dir;
        }
    }
}

fn setup(mut commands: Commands, mut grid: ResMut<Grid>) {
    commands.spawn().insert(Cursor {
        limits: (Limit::new(0, 80), Limit::new(0, 25)),
        ..Default::default()
    });

    for _ in 0..80 {
        let mut col = Vec::default();
        for _ in 0..25 {
            col.push(Cell {
                value: CellValue::Empty,
                heat: 0,
            });
        }
        grid.grid.push(col);
    }
}

fn run_step(mut grid: ResMut<Grid>, mut query: Query<&mut Cursor>) {
    for cursor in query.iter_mut() {
        handle_cell(&mut grid, cursor.into_inner());
    }
}

fn handle_cell(grid: &mut Grid, cursor: &mut Cursor) {
    let cell = grid.get(cursor.x as usize, cursor.y as usize);
    match cell.value {
        CellValue::Number(num) => cursor.stack.push(num),
        CellValue::Dir(direction) => cursor.direction = direction,
        CellValue::Op(operator) =>
            match operator {
                Operator::Nullary(op) =>
                    match op {
                        Input::Integer => todo!(),
                        Input::Ascii => todo!(),
                    }
                Operator::Unary(op) => {
                    if cursor.stack.is_empty() {
                        cursor.stack.push(0);
                    }

                    match op {
                        UnaryOperator::Pop => { cursor.stack.pop(); },
                        UnaryOperator::Negate => { let v = cursor.stack.pop().unwrap(); cursor.stack.push(- v); },
                        _ => todo!()
                    }
                }
                Operator::Binary(op) => todo!(),
                Operator::Ternary(op) => todo!(),
            }
        _ => todo!()
    }
}

pub struct GridPlugin;
impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::default())
            .add_startup_system(setup);
    }
}
