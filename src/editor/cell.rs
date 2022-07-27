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
    Char(char),
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

#[derive(Clone, Copy)]
pub enum Operator {
    Nullary(NullaryOperator),
    Unary(UnaryOperator),
    Binary(BinaryOperator),
    Ternary(TernaryOperator),
}

impl From<Operator> for char {
    fn from(value: Operator) -> Self {
        match value {
            Operator::Nullary(op) => op.into(),
            Operator::Unary(op) => op.into(),
            Operator::Binary(op) => op.into(),
            Operator::Ternary(op) => op.into(),
        }
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

impl From<Direction> for char {
    fn from(value: Direction) -> char {
        match value {
            Direction::Up => '^',
            Direction::Down => 'v',
            Direction::Left => '<',
            Direction::Right => '>',
            Direction::Random => '?',
        }
    }
}

#[derive(Clone, Copy)]
pub enum NullaryOperator {
    Integer,
    Ascii,
}

impl From<NullaryOperator> for char {
    fn from(value: NullaryOperator) -> char {
        match value {
            NullaryOperator::Integer => '&',
            NullaryOperator::Ascii => '~',
        }
    }
}

#[derive(Clone, Copy)]
pub enum UnaryOperator {
    Negate,
    Duplicate,
    Pop,
    WriteNumber,
    WriteASCII,
}

impl From<UnaryOperator> for char {
    fn from(value: UnaryOperator) -> char {
        match value {
            UnaryOperator::Negate => '!',
            UnaryOperator::Duplicate => ':',
            UnaryOperator::Pop => '$',
            UnaryOperator::WriteNumber => '.',
            UnaryOperator::WriteASCII => ',',
        }
    }
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

impl From<BinaryOperator> for char {
    fn from(value: BinaryOperator) -> char {
        match value {
            BinaryOperator::Greater => '`',
            BinaryOperator::Add => '+',
            BinaryOperator::Subtract => '-',
            BinaryOperator::Multiply => '*',
            BinaryOperator::Divide => '/',
            BinaryOperator::Modulo => '%',
            BinaryOperator::Swap => '\\',
            BinaryOperator::Get => 'g',
        }
    }
}

#[derive(Clone, Copy)]
pub enum TernaryOperator {
    Put,
}

impl From<TernaryOperator> for char {
    fn from(value: TernaryOperator) -> char {
        match value {
            TernaryOperator::Put => 'p',
        }
    }
}

#[derive(Clone, Copy)]
pub enum IfDir {
    Horizontal,
    Vertical,
}

impl From<IfDir> for char {
    fn from(value: IfDir) -> char {
        match value {
            IfDir::Horizontal => '_',
            IfDir::Vertical => '|',
        }
    }
}
