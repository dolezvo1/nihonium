
use crate::common::search::ast::Expr;
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(search, "/common/search/search.rs");

pub fn parse(input: &str) -> Result<Expr, String> {
    search::ExprParser::new()
        .parse(input)
        .map_err(|e| e.to_string())
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_literal() {
        let e = parse(r#""foo""#).unwrap();
        assert_eq!(e, Expr::Literal("foo".to_owned()))
    }

    #[test]
    fn test_and1() {
        let e = parse(r#""foo" AND "bar""#).unwrap();
        assert_eq!(
            e,
            Expr::And(
                Box::new(Expr::Literal("foo".to_owned())),
                Box::new(Expr::Literal("bar".to_owned())),
            )
        )
    }

    #[test]
    fn test_and2() {
        let e = parse(r#"("foo" AND "bar")"#).unwrap();
        assert_eq!(
            e,
            Expr::And(
                Box::new(Expr::Literal("foo".to_owned())),
                Box::new(Expr::Literal("bar".to_owned())),
            )
        )
    }

    #[test]
    fn test_implicit_and() {
        let e = parse(r#"("foo" OR "bar") (NOT "baz")"#).unwrap();
        assert_eq!(
            e,
            Expr::And(
                Box::new(Expr::Or(
                    Box::new(Expr::Literal("foo".to_owned())),
                    Box::new(Expr::Literal("bar".to_owned())),
                )),
                Box::new(Expr::Not(
                    Box::new(Expr::Literal("baz".to_owned())),
                )),
            )
        )
    }
}

