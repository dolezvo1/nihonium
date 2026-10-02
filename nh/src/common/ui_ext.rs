use eframe::egui::{
    Layout, Response, TextBuffer, TextEdit, Ui, WidgetText,
    text::{CCursor, CCursorRange},
};

pub trait UiExt {
    fn labeled_text_edit_singleline<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
    ) -> Response;
    fn labeled_text_edit_singleline2<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
        focus_and_select: bool,
    ) -> Response;
    fn labeled_text_edit_multiline<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
    ) -> Response;
    fn labeled_text_edit_multiline2<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
        focus_and_select: bool,
    ) -> Response;
}

impl UiExt for Ui {
    fn labeled_text_edit_singleline<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
    ) -> Response {
        self.label(label);
        self.add_sized((self.available_width(), 20.0), TextEdit::singleline(edit))
    }
    fn labeled_text_edit_singleline2<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
        focus_and_select: bool,
    ) -> Response {
        self.label(label);

        let layout = Layout::centered_and_justified(self.layout().main_dir());
        let mut output = self
            .allocate_ui_with_layout((self.available_width(), 20.0).into(), layout, |ui| {
                TextEdit::singleline(edit).show(ui)
            })
            .inner;
        if focus_and_select {
            output.response.request_focus();

            let last = CCursor::new(edit.as_str().chars().count());
            let selection = CCursorRange::two(CCursor::new(0), last);
            output.state.cursor.set_char_range(Some(selection));
            output.state.store(self.ctx(), output.response.id);
        }
        output.response.response
    }

    fn labeled_text_edit_multiline<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
    ) -> Response {
        self.label(label);
        self.add_sized((self.available_width(), 20.0), TextEdit::multiline(edit))
    }
    fn labeled_text_edit_multiline2<S: TextBuffer>(
        &mut self,
        label: impl Into<WidgetText>,
        edit: &mut S,
        focus_and_select: bool,
    ) -> Response {
        self.label(label);

        let layout = Layout::centered_and_justified(self.layout().main_dir());
        let mut output = self
            .allocate_ui_with_layout((self.available_width(), 20.0).into(), layout, |ui| {
                TextEdit::multiline(edit).show(ui)
            })
            .inner;
        if focus_and_select {
            output.response.request_focus();

            let last = CCursor::new(edit.as_str().chars().count());
            let selection = CCursorRange::two(CCursor::new(0), last);
            output.state.cursor.set_char_range(Some(selection));
            output.state.store(self.ctx(), output.response.id);
        }
        output.response.response
    }
}
