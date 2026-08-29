use crate::common::uuid::{ModelUuid, ResourceUuid, ViewUuid};

pub mod ast;
pub mod parser;

pub trait FullTextSearchable {
    fn full_text_search(&self, acc: &mut ModelSearcher);
}

pub struct ModelSearcher<'a> {
    expr: &'a ast::Expr,
    current_component: ModelUuid,
    current_found_matches: Vec<ModelUuid>,
    completed_components: Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)>,
}

impl<'a> ModelSearcher<'a> {
    pub fn new(expr: &'a ast::Expr) -> Self {
        Self {
            expr,
            current_component: ModelUuid::nil(),
            current_found_matches: Vec::new(),
            completed_components: Vec::new(),
        }
    }

    pub fn open_component(&mut self, uuid: ModelUuid) {
        self.current_component = uuid;
    }
    pub fn close_component(&mut self, views: Vec<ViewUuid>) {
        let cfm = std::mem::take(&mut self.current_found_matches);
        if !cfm.is_empty() {
            self.completed_components
                .push((self.current_component, cfm, views));
        }
    }

    pub fn check_element(&mut self, uuid: ModelUuid, fields: &[&str]) {
        if check_model(&self.expr, fields) {
            self.current_found_matches.push(uuid);
        }
    }

    pub fn results(self) -> Vec<(ModelUuid, Vec<ModelUuid>, Vec<ViewUuid>)> {
        self.completed_components
    }
}

fn check_model(expr: &ast::Expr, fields: &[&str]) -> bool {
    match expr {
        ast::Expr::Literal(s) => fields.iter().any(|e| e.contains(s)),
        ast::Expr::Not(expr) => !check_model(expr, fields),
        ast::Expr::Or(lhs, rhs) => check_model(lhs, fields) || check_model(rhs, fields),
        ast::Expr::And(lhs, rhs) => check_model(lhs, fields) && check_model(rhs, fields),
    }
}

pub struct ResourceSearcher<'a> {
    expr: &'a ast::Expr,
    found_matches: Vec<(ResourceUuid, Vec<(usize, String)>)>,
}

impl<'a> ResourceSearcher<'a> {
    pub fn new(expr: &'a ast::Expr) -> Self {
        Self {
            expr,
            found_matches: Vec::new(),
        }
    }

    pub fn check_resource(&mut self, uuid: ResourceUuid, contents: &str) {
        if check_resource(&self.expr, contents) {
            self.found_matches.push((uuid, Vec::new()));
        }
    }

    pub fn results(self) -> Vec<(ResourceUuid, Vec<(usize, String)>)> {
        self.found_matches
    }
}

fn check_resource(expr: &ast::Expr, contents: &str) -> bool {
    match expr {
        ast::Expr::Literal(s) => contents.contains(s),
        ast::Expr::Not(expr) => !check_resource(expr, contents),
        ast::Expr::Or(lhs, rhs) => check_resource(lhs, contents) || check_resource(rhs, contents),
        ast::Expr::And(lhs, rhs) => check_resource(lhs, contents) && check_resource(rhs, contents),
    }
}
