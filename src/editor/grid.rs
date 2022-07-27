use super::cell::{self, Cell, CellValue, Direction};
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

    pub fn shift(&mut self, dir: Direction, amount: i32) {
        match dir {
            Direction::Up => self.transpose(0, amount),
            Direction::Down => self.transpose(0, -amount),
            Direction::Left => self.transpose(-amount, 0),
            Direction::Right => self.transpose(amount, 0),
            Direction::Random => {
                let new_dir = match (rand::random::<bool>(), rand::random::<bool>()) {
                    (false, false) => Direction::Down,
                    (false, true) => Direction::Up,
                    (true, false) => Direction::Left,
                    (true, true) => Direction::Right,
                };

                self.shift(new_dir, amount);
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
        CellValue::Op(operator) => match operator {
            cell::Operator::Nullary(op) => match op {
                cell::NullaryOperator::Integer => todo!(),
                cell::NullaryOperator::Ascii => todo!(),
            },
            cell::Operator::Unary(op) => {
                if cursor.stack.is_empty() {
                    cursor.stack.push(0);
                }

                match op {
                    cell::UnaryOperator::Pop => {
                        cursor.stack.pop();
                    }
                    cell::UnaryOperator::Negate => {
                        let v = cursor.stack.pop().unwrap();
                        cursor.stack.push((v == 0) as i32);
                    }
                    cell::UnaryOperator::Duplicate => {
                        let v = cursor.stack.pop().unwrap();
                        cursor.stack.push(v);
                        cursor.stack.push(v);
                    }
                    cell::UnaryOperator::WriteNumber => {
                        warn!(
                            "cell::UnaryOperator::WriteNumber `{}`",
                            cursor.stack.pop().unwrap()
                        );
                        todo!();
                    }
                    cell::UnaryOperator::WriteASCII => {
                        warn!(
                            "cell::UnaryOperator::WriteASCII `{}`",
                            cursor.stack.pop().unwrap()
                        );
                        todo!();
                    }
                }
            }
            cell::Operator::Binary(op) => {
                let stack_len = cursor.stack.len();
                if stack_len < 2 {
                    cursor.stack.extend(vec![0; 2 - stack_len]);
                }

                let b = cursor.stack.pop().unwrap();
                let a = cursor.stack.pop().unwrap();

                match op {
                    cell::BinaryOperator::Greater => {
                        cursor.stack.push((a > b) as i32);
                    }
                    cell::BinaryOperator::Add => {
                        cursor.stack.push(a + b);
                    }
                    cell::BinaryOperator::Subtract => {
                        cursor.stack.push(a - b);
                    }
                    cell::BinaryOperator::Multiply => {
                        cursor.stack.push(a * b);
                    }
                    cell::BinaryOperator::Divide => {
                        cursor.stack.push(if b != 0 { a / b } else { 0 });
                    }
                    cell::BinaryOperator::Modulo => {
                        cursor.stack.push(if b != 0 { a % b } else { 0 });
                    }
                    cell::BinaryOperator::Swap => {
                        cursor.stack.push(a);
                        cursor.stack.push(b);
                    }
                    cell::BinaryOperator::Get => {
                        cursor
                            .stack
                            .push(char::from(grid.get(a as usize, b as usize).value) as i32);
                    }
                }
            }
            cell::Operator::Ternary(op) => todo!(),
        },
        _ => todo!(),
    }
}

pub struct GridPlugin;
impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Grid::default())
            .add_startup_system(setup);
    }
}
