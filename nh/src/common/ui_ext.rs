
use eframe::egui::{Response, TextBuffer, TextEdit, Ui, WidgetText};

pub trait UiExt {
    fn labeled_text_edit_singleline<S: TextBuffer>(&mut self, label: impl Into<WidgetText>, edit: &mut S) -> Response;
    fn labeled_text_edit_multiline<S: TextBuffer>(&mut self, label: impl Into<WidgetText>, edit: &mut S) -> Response;
}

impl UiExt for Ui {
    fn labeled_text_edit_singleline<S: TextBuffer>(&mut self, label: impl Into<WidgetText>, edit: &mut S) -> Response {
        self.label(label);
        self.add_sized(
            (self.available_width(), 20.0),
            TextEdit::singleline(edit),
        )
    }

    fn labeled_text_edit_multiline<S: TextBuffer>(&mut self, label: impl Into<WidgetText>, edit: &mut S) -> Response {
        self.label(label);
        self.add_sized(
            (self.available_width(), 20.0),
            TextEdit::multiline(edit),
        )
    }
}
