use crate::common::uuid::{ModelUuid, ViewUuid};

pub mod ast;
pub mod parser;

pub trait FullTextSearchable {
    fn full_text_search(&self, acc: &mut Searcher);
}


pub struct Searcher {
    current_diagram: ViewUuid,
    expr: ast::Expr,
    found_matches: Vec<(ViewUuid, ModelUuid)>,
}

impl Searcher {
    pub fn new(expr: ast::Expr) -> Self {
        Self {
            current_diagram: ViewUuid::nil(),
            expr,
            found_matches: Vec::new(),
        }
    }

    pub fn set_current_diagram(
        &mut self,
        uuid: ViewUuid,
    ) {
        self.current_diagram = uuid;
    }

    pub fn check_element(
        &mut self,
        uuid: ModelUuid,
        fields: &[&str],
    ) {
        if check(&self.expr, fields) {
            self.found_matches.push((self.current_diagram, uuid));
        }
    }

    pub fn results(self) -> Vec<(ViewUuid, ModelUuid)> {
        self.found_matches
    }
}


fn check(expr: &ast::Expr, fields: &[&str]) -> bool {
    match expr {
        ast::Expr::Literal(s) => fields.iter().any(|e| e.contains(s)),
        ast::Expr::Not(expr) => !check(expr, fields),
        ast::Expr::Or(lhs, rhs) => check(lhs, fields) || check(rhs, fields),
        ast::Expr::And(lhs, rhs) => check(lhs, fields) && check(rhs, fields),
    }
}
