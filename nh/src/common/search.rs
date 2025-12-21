use crate::common::uuid::{ModelUuid, ViewUuid};

pub mod ast;
pub mod parser;

pub trait FullTextSearchable {
    fn full_text_search(&self, acc: &mut Searcher);
}


pub struct Searcher {
    current_diagrams: Vec<ViewUuid>,
    expr: ast::Expr,
    found_matches: Vec<(ModelUuid, Vec<ViewUuid>)>,
}

impl Searcher {
    pub fn new(expr: ast::Expr) -> Self {
        Self {
            current_diagrams: Vec::new(),
            expr,
            found_matches: Vec::new(),
        }
    }

    pub fn set_current_diagrams(
        &mut self,
        uuids: Vec<ViewUuid>,
    ) {
        self.current_diagrams = uuids;
    }

    pub fn check_element(
        &mut self,
        uuid: ModelUuid,
        fields: &[&str],
    ) {
        if check(&self.expr, fields) {
            self.found_matches.push((uuid, self.current_diagrams.clone()));
        }
    }

    pub fn results(self) -> Vec<(ModelUuid, Vec<ViewUuid>)> {
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
