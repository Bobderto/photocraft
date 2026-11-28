//! #16844: the canvas text editor keeps Tab while it has focus.

use egui::accesskit::Role;
use egui::{Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::PhotocraftApp;
use crate::shortcut_dispatch::tests::{app_harness, tap};
use crate::state::Tool;

fn harness(tool: Tool) -> Harness<'static, Box<PhotocraftApp>> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app.ui.tool = tool;
    app_harness(app)
}

fn start_typing(h: &mut Harness<'_, Box<PhotocraftApp>>) {
    let pos = h.state().last_canvas_rect.center();
    h.hover_at(pos);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
    h.event(egui::Event::Text("ab".into()));
    h.run_steps(2);
    assert!(editor_focused(h), "the Type editor takes keyboard focus");
}

fn editor_focused(h: &Harness<'_, Box<PhotocraftApp>>) -> bool {
    h.query_all_by_role(Role::MultilineTextInput).any(|node| node.is_focused())
}

fn panels_shown(h: &Harness<'_, Box<PhotocraftApp>>) -> bool {
    h.query_all_by_label("Layers").next().is_some()
}

#[test]
fn tab_types_once_without_hiding_panels_in_all_four_type_tools() {
    for tool in [Tool::Type, Tool::VerticalType, Tool::HorizontalTypeMask, Tool::VerticalTypeMask] {
        let mut h = harness(tool);
        start_typing(&mut h);
        assert!(panels_shown(&h), "{tool:?}: panels start visible");

        tap(&mut h, Key::Tab, Modifiers::NONE);
        h.event(egui::Event::Text("cd".into()));
        h.run_steps(2);
        let text = h.state_mut().run("type.info", json!({})).unwrap();
        assert_eq!(text["text"], "ab\tcd", "{tool:?}: the editor inserts one tab");
        assert!(editor_focused(&h), "{tool:?}: the editor keeps focus");
        assert!(panels_shown(&h), "{tool:?}: typing Tab leaves the panels visible");
    }
}

#[test]
fn tab_toggles_panels_after_commit_or_escape() {
    for (key, mods) in [(Key::Enter, Modifiers::COMMAND), (Key::Escape, Modifiers::NONE)] {
        let mut h = harness(Tool::Type);
        start_typing(&mut h);
        tap(&mut h, key, mods);
        assert!(h.state().ui.text_edit.is_none(), "{key:?}: editing ended");

        assert!(panels_shown(&h), "{key:?}: panels start visible");
        tap(&mut h, Key::Tab, Modifiers::NONE);
        assert!(!panels_shown(&h), "{key:?}: Tab hides the panels");
        tap(&mut h, Key::Tab, Modifiers::NONE);
        assert!(panels_shown(&h), "{key:?}: Tab shows the panels again");
    }
}
