use crate::common::uuid::{ModelUuid, ViewUuid};

pub mod ast;
pub mod parser;

pub trait FullTextSearchable {
    fn full_text_search(&self, acc: &mut Searcher);
}


pub struct Searcher {
    expr: ast::Expr,
    current_component: ModelUuid,
    current_found_matches: Vec<ModelUuid>,
    completed_components: Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>
}

impl Searcher {
    pub fn new(expr: ast::Expr) -> Self {
        Self {
            expr,
            current_component: ModelUuid::nil(),
            current_found_matches: Vec::new(),
            completed_components: Vec::new(),
        }
    }

    pub fn open_component(
        &mut self,
        uuid: ModelUuid,
    ) {
        self.current_component = uuid;
    }
    pub fn close_component(
        &mut self,
        views: Vec<ViewUuid>,
    ) {
        let cfm = std::mem::take(&mut self.current_found_matches);
        if !cfm.is_empty() {
            self.completed_components.push((self.current_component, cfm, views));
        }
    }

    pub fn check_element(
        &mut self,
        uuid: ModelUuid,
        fields: &[&str],
    ) {
        if check(&self.expr, fields) {
            self.current_found_matches.push(uuid);
        }
    }

    pub fn results(self) -> Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)> {
        self.completed_components
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
