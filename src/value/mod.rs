#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum Value {
    Unit,
    Int(i64),
    Bool(bool),
    Char(char),
}
