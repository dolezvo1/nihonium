
#[derive(Debug, PartialEq)]
pub enum Expr {
    Literal(String),
    Not(Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
}
