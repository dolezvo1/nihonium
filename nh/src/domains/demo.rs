
use crate::egui;

pub const EXTERNAL_ROLE_BACKGROUND: egui::Color32 = egui::Color32::LIGHT_GRAY;
pub const INTERNAL_ROLE_BACKGROUND: egui::Color32 = egui::Color32::WHITE;

pub const PERFORMA_DETAIL: egui::Color32 = egui::Color32::RED;
pub const INFORMA_DETAIL: egui::Color32 = egui::Color32::from_rgb(0, 175, 0);
pub const FORMA_DETAIL: egui::Color32 = egui::Color32::BLUE;

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DemoTransactionKind {
    Performa,
    Informa,
    Forma
}

impl DemoTransactionKind {
    pub fn char(&self) -> &'static str {
        match self {
            Self::Performa => "Performa",
            Self::Informa => "Informa",
            Self::Forma => "Forma",
        }
    }
}

impl Default for DemoTransactionKind {
    fn default() -> Self {
        Self::Performa
    }
}
