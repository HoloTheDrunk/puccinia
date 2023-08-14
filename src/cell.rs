use anyhow::anyhow;
use tui::{
    style::{Color, Style},
    text::Span,
};

/// Represents a single cell of the grid.
#[derive(Clone, Debug, Default, Copy)]
pub struct Cell {
    /// The content of the cell.
    pub value: CellValue,
    /// Heat represents how recently the cell was last "visited" by a cursor.
    pub heat: u8,
    pub is_breakpoint: bool,
}

impl From<CellValue> for Cell {
    fn from(value: CellValue) -> Self {
        Cell {
            value,
            heat: 0,
            is_breakpoint: false,
        }
    }
}

impl From<char> for Cell {
    fn from(value: char) -> Self {
        Cell::from(CellValue::from(value))
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Default, Copy)]
pub enum CellValue {
    #[default]
    Empty,
    Op(Operator),
    Dir(Direction),
    If(IfDir),
    StringMode,
    Bridge,
    End,
    Number(u32),
    Char(char),
}

impl From<char> for CellValue {
    fn from(value: char) -> Self {
        match value {
            ' ' => CellValue::Empty,
            '\"' => CellValue::StringMode,
            '#' => CellValue::Bridge,
            '@' => CellValue::End,
            v @ '0'..='9' => CellValue::Number(v.to_digit(10).unwrap()),
            c => {
                if let Ok(op) = Operator::try_from(c) {
                    CellValue::Op(op)
                } else if let Ok(dir) = Direction::try_from(c) {
                    CellValue::Dir(dir)
                } else if let Ok(ifdir) = IfDir::try_from(c) {
                    CellValue::If(ifdir)
                } else {
                    CellValue::Char(c)
                }
            }
        }
    }
}

impl From<CellValue> for char {
    fn from(value: CellValue) -> Self {
        match value {
            CellValue::Empty => ' ',
            CellValue::Op(operator) => operator.into(),
            CellValue::Dir(dir) => dir.into(),
            CellValue::If(dir) => dir.into(),
            CellValue::StringMode => '"',
            CellValue::Bridge => '#',
            CellValue::End => '@',
            CellValue::Number(num) => num.to_string().chars().next().unwrap(),
            CellValue::Char(c) => c,
        }
    }
}

impl<'s> From<&Cell> for Span<'s> {
    fn from(cell: &Cell) -> Self {
        Span::styled(char::from(cell.value).to_string(), cell.into())
    }
}

impl From<&Cell> for Style {
    fn from(cell: &Cell) -> Self {
        Style::default()
            .fg(match cell.value {
                CellValue::Empty => Color::Reset,
                CellValue::Op(op) => op.into(),
                CellValue::Dir(dir) => dir.into(),
                CellValue::If(cond) => cond.into(),
                CellValue::StringMode => Color::Cyan,
                CellValue::Bridge => Color::LightGreen,
                CellValue::End => Color::Cyan,
                CellValue::Number(_) => Color::Magenta,
                CellValue::Char(_) => Color::White,
            })
            .bg(if cell.heat > 64 {
                Color::Rgb((128. * (cell.heat as f32 / 128 as f32)) as u8, 0, 0)
            } else {
                Color::Reset
            })
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
pub enum Operator {
    Nullary(NullaryOperator),
    Unary(UnaryOperator),
    Binary(BinaryOperator),
    Ternary(TernaryOperator),
}

impl TryFrom<char> for Operator {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        Ok(if let Ok(nullary) = NullaryOperator::try_from(value) {
            Operator::Nullary(nullary)
        } else if let Ok(unary) = UnaryOperator::try_from(value) {
            Operator::Unary(unary)
        } else if let Ok(binary) = BinaryOperator::try_from(value) {
            Operator::Binary(binary)
        } else if let Ok(ternary) = TernaryOperator::try_from(value) {
            Operator::Ternary(ternary)
        } else {
            return Err(anyhow!("Invalid operator `{value}`"));
        })
    }
}

impl From<Operator> for char {
    fn from(value: Operator) -> Self {
        match value {
            Operator::Nullary(nullary) => char::from(nullary),
            Operator::Unary(unary) => char::from(unary),
            Operator::Binary(binary) => char::from(binary),
            Operator::Ternary(ternary) => char::from(ternary),
        }
    }
}

impl From<Operator> for Color {
    fn from(value: Operator) -> Self {
        match value {
            Operator::Nullary(nullary) => Color::from(nullary),
            Operator::Unary(unary) => Color::from(unary),
            Operator::Binary(binary) => Color::from(binary),
            Operator::Ternary(ternary) => Color::from(ternary),
        }
    }
}

macro_rules! char_mapping {
    ($($enum:ident : $($variant:ident = $c:literal => $color:ident),* $(,)?);* $(;)?) => {
        $(
            impl TryFrom<char> for $enum {
                type Error = anyhow::Error;

                fn try_from(value: char) -> Result<Self, Self::Error> {
                    Ok(match value {
                        $(
                            $c => $enum::$variant,
                        )*
                        c => return Err(anyhow!("Invalid {} `{}`", stringify!($enum), c)),
                    })
                }
            }

            impl From<$enum> for char {
                fn from(value: $enum) -> char {
                    match value {
                        $(
                            $enum::$variant => $c,
                        )*
                    }
                }
            }

            impl From<$enum> for Color {
                fn from(value: $enum) -> Color {
                    match value {
                        $(
                            $enum::$variant => Color::$color,
                        )*
                    }
                }
            }
        )*
    };
}

// FIXME: Broken Green color, may be due to terminal theme
char_mapping! {
    NullaryOperator:
        Integer = '&' => Red,
        Ascii = '~' => Red;

    UnaryOperator:
        Negate = '!' => Yellow,
        Duplicate = ':' => LightRed,
        Pop = '$' => LightRed,
        WriteNumber = '.' => Red,
        WriteASCII = ',' => Red;

    BinaryOperator:
        Greater = '`' => Green,
        Add = '+' => Yellow,
        Subtract = '-' => Yellow,
        Multiply = '*' => Yellow,
        Divide = '/' => Yellow,
        Modulo = '%' => Yellow,
        Swap = '\\' => LightRed,
        Get = 'g' => Magenta;

    TernaryOperator:
        Put = 'p' => Magenta;

    IfDir:
        Horizontal = '_' => Green,
        Vertical = '|' => Green;

    Direction:
        Up = '^' => LightGreen,
        Down = 'v' => LightGreen,
        Left = '<' => LightGreen,
        Right = '>' => LightGreen,
        Random = '?' => LightGreen;
}

#[cfg_attr(test, derive(Hash, Eq))]
#[derive(Default, PartialEq, Clone, Debug, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    #[default]
    Right,
    Random,
}

impl std::ops::Neg for Direction {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Random => self,
        }
    }
}

impl From<(i32, i32)> for Direction {
    fn from(value: (i32, i32)) -> Self {
        let (x, y) = value;
        match (x.signum(), y.signum()) {
            (0, -1) => Self::Up,
            (0, 1) => Self::Down,
            (-1, 0) => Self::Left,
            (1, 0) => Self::Right,
            _ => panic!("Invalid direction {value:?}"),
        }
    }
}

impl From<Direction> for (i32, i32) {
    fn from(val: Direction) -> Self {
        match val {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            Direction::Random => match (rand::random::<bool>(), rand::random::<bool>()) {
                (false, false) => Direction::Down,
                (false, true) => Direction::Up,
                (true, false) => Direction::Left,
                (true, true) => Direction::Right,
            }
            .into(),
        }
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
pub enum NullaryOperator {
    Integer,
    Ascii,
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
pub enum UnaryOperator {
    Negate,
    Duplicate,
    Pop,
    WriteNumber,
    WriteASCII,
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
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

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
pub enum TernaryOperator {
    Put,
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy)]
pub enum IfDir {
    Horizontal,
    Vertical,
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! collection {
        ($($k:expr => $v:expr),* $(,)?) => {{
            core::convert::From::from([$(($k, $v),)*])
        }};
    }

    #[test]
    fn serialize() {
        let map: Vec<(CellValue, char)> = collection! {
            CellValue::Empty => ' ',
            CellValue::Op(Operator::Nullary(NullaryOperator::Integer)) => '&',
            CellValue::Op(Operator::Nullary(NullaryOperator::Ascii)) => '~',
            CellValue::Op(Operator::Unary(UnaryOperator::Negate)) => '!',
            CellValue::Op(Operator::Unary(UnaryOperator::Duplicate)) => ':',
            CellValue::Op(Operator::Unary(UnaryOperator::Pop)) => '$',
            CellValue::Op(Operator::Unary(UnaryOperator::WriteNumber)) => '.',
            CellValue::Op(Operator::Unary(UnaryOperator::WriteASCII)) => ',',
            CellValue::Op(Operator::Binary(BinaryOperator::Greater)) => '`',
            CellValue::Op(Operator::Binary(BinaryOperator::Add)) => '+',
            CellValue::Op(Operator::Binary(BinaryOperator::Subtract)) => '-',
            CellValue::Op(Operator::Binary(BinaryOperator::Multiply)) => '*',
            CellValue::Op(Operator::Binary(BinaryOperator::Divide)) => '/',
            CellValue::Op(Operator::Binary(BinaryOperator::Modulo)) => '%',
            CellValue::Op(Operator::Binary(BinaryOperator::Swap)) => '\\',
            CellValue::Op(Operator::Binary(BinaryOperator::Get)) => 'g',
            CellValue::Op(Operator::Ternary(TernaryOperator::Put)) => 'p',
            CellValue::Dir(Direction::Up) => '^',
            CellValue::Dir(Direction::Down) => 'v',
            CellValue::Dir(Direction::Left) => '<',
            CellValue::Dir(Direction::Right) => '>',
            CellValue::If(IfDir::Horizontal) => '_',
            CellValue::If(IfDir::Vertical) => '|',
            CellValue::StringMode => '"',
            CellValue::Bridge => '#',
            CellValue::End => '@',
            CellValue::Number(5) => '5',
            CellValue::Char('c') => 'c',
        };

        for (cell_value, expected) in map.iter() {
            let got = char::from(*cell_value);
            assert_eq!(*expected, got, "Failed to serialize {cell_value:?}: {got}",);
        }
    }
}
