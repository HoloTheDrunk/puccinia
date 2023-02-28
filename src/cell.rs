use serde::{Deserialize, Serialize};

/// Represents a single cell of the grid.
#[derive(Clone, Debug, Copy)]
pub struct Cell {
    /// The content of the cell
    pub value: CellValue,
    /// Heat represents how long ago the cell was last "visited" by a cursor.
    pub heat: i8,
}

impl From<CellValue> for Cell {
    fn from(value: CellValue) -> Self {
        Cell { value, heat: 0 }
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum CellValue {
    #[serde(rename = " ")]
    Empty,
    Op(Operator),
    Dir(Direction),
    If(IfDir),
    #[serde(rename = "\"")]
    StringMode,
    #[serde(rename = "#")]
    Bridge,
    #[serde(rename = "@")]
    End,
    Number(u32),
    Char(char),
}

impl From<char> for CellValue {
    fn from(value: char) -> Self {
        serde_json::from_str(value.to_string().as_ref())
            .expect(format!("Invalid cell value `{value}`").as_ref())
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

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Operator {
    Nullary(NullaryOperator),
    Unary(UnaryOperator),
    Binary(BinaryOperator),
    Ternary(TernaryOperator),
}

impl From<Operator> for char {
    #[inline]
    fn from(value: Operator) -> Self {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, Eq))]
#[derive(Default, PartialEq, Clone, Debug, Copy, Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "^")]
    Up,
    #[serde(rename = "v")]
    Down,
    #[serde(rename = "<")]
    Left,
    #[default]
    #[serde(rename = ">")]
    Right,
    #[serde(rename = "?")]
    Random,
}

impl From<Direction> for char {
    #[inline]
    fn from(value: Direction) -> char {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub enum NullaryOperator {
    #[serde(rename = "&")]
    Integer,
    #[serde(rename = "~")]
    Ascii,
}

impl From<NullaryOperator> for char {
    #[inline]
    fn from(value: NullaryOperator) -> char {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub enum UnaryOperator {
    #[serde(rename = "!")]
    Negate,
    #[serde(rename = ":")]
    Duplicate,
    #[serde(rename = "$")]
    Pop,
    #[serde(rename = ".")]
    WriteNumber,
    #[serde(rename = ",")]
    WriteASCII,
}

impl From<UnaryOperator> for char {
    #[inline]
    fn from(value: UnaryOperator) -> char {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub enum BinaryOperator {
    #[serde(rename = "`")]
    Greater,
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Subtract,
    #[serde(rename = "*")]
    Multiply,
    #[serde(rename = "/")]
    Divide,
    #[serde(rename = "%")]
    Modulo,
    #[serde(rename = "\\")]
    Swap,
    #[serde(rename = "g")]
    Get,
}

impl From<BinaryOperator> for char {
    #[inline]
    fn from(value: BinaryOperator) -> char {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub enum TernaryOperator {
    #[serde(rename = "p")]
    Put,
}

impl From<TernaryOperator> for char {
    #[inline]
    fn from(value: TernaryOperator) -> char {
        to_json_char(value)
    }
}

#[cfg_attr(test, derive(Hash, PartialEq, Eq))]
#[derive(Clone, Debug, Copy, Deserialize, Serialize)]
pub enum IfDir {
    #[serde(rename = "_")]
    Horizontal,
    #[serde(rename = "|")]
    Vertical,
}

impl From<IfDir> for char {
    #[inline]
    fn from(value: IfDir) -> char {
        to_json_char(value)
    }
}

fn to_json_char<T: Serialize>(value: T) -> char {
    serde_json::to_string(&value)
        .unwrap()
        .chars()
        .nth(1)
        .unwrap()
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
            assert_eq!(
                *expected,
                char::from(*cell_value),
                "Failed to serialize {cell_value:?}: {}",
                serde_json::to_value(cell_value).unwrap()
            );
        }
    }
}
