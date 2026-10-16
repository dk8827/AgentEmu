//! Expose egui's actual semantic tree to browser assistive technology.
//! Actions return through AccessKit / egui input, not a second application model.
use egui::{
    accesskit::{self, Action, ActionData, ActionRequest, NodeId},
    *,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Default)]
pub(crate) struct Bridge {
    nodes: HashMap<u64, accesskit::Node>,
    pub json: String,
    pub modal: Option<String>,
    pub browser_focus: Option<bool>,
    events: Vec<Event>,
}
pub(crate) type SharedBridge = Arc<Mutex<Bridge>>;
pub(crate) struct Plugin(pub SharedBridge);
impl egui::Plugin for Plugin {
    fn debug_name(&self) -> &'static str {
        "browser-accessibility"
    }
    fn setup(&mut self, ctx: &Context) {
        ctx.enable_accesskit();
    }
    fn input_hook(&mut self, input: &mut RawInput) {
        if let Ok(mut b) = self.0.lock() {
            if let Some(focused) = b.browser_focus {
                input.focused = focused;
            }
            input.events.append(&mut b.events);
        }
    }
    fn output_hook(&mut self, output: &mut FullOutput) {
        let Some(tree) = &output.platform_output.accesskit_update else {
            return;
        };
        let Ok(mut bridge) = self.0.lock() else {
            return;
        };
        bridge.nodes = tree.nodes.iter().map(|(id, n)| (id.0, n.clone())).collect();
        let nodes:Vec<_>=tree.nodes.iter().map(|(id,n)|serde_json::json!({
            "id":id.0.to_string(),"role":format!("{:?}",n.role()),"label":n.label(),"value":n.value(),
            "disabled":n.is_disabled(),"hidden":n.is_hidden(),"selected":n.is_selected(),"checked":n.toggled().map(|t|format!("{t:?}")),
            "click":n.supports_action(Action::Click),"focusable":n.supports_action(Action::Focus),
            "number":n.numeric_value(),"min":n.min_numeric_value(),"max":n.max_numeric_value(),"step":n.numeric_value_step(),
            "bounds":n.bounds().map(|r|[r.x0,r.y0,r.x1-r.x0,r.y1-r.y0]),
            "children":n.children().iter().map(|c|c.0.to_string()).collect::<Vec<_>>()
        })).collect();
        bridge.json=serde_json::json!({"modal":bridge.modal,"pixels_per_point":output.pixels_per_point,"nodes":nodes,"root":tree.tree.as_ref().map(|t|t.root.0.to_string()),"focus":tree.focus.0.to_string()}).to_string();
    }
}
impl Bridge {
    pub fn action(&mut self, id: &str, action: &str, value: &str) -> bool {
        let Ok(id) = id.parse::<u64>() else {
            return false;
        };
        let Some(node) = self.nodes.get(&id) else {
            return false;
        };
        if node.is_disabled() || node.is_hidden() {
            return false;
        }
        if action == "escape" {
            self.events.push(Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            self.events.push(Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            return true;
        }
        let (action, data) = match action {
            "click" => (Action::Click, None),
            "focus" => (Action::Focus, None),
            "increment" => (Action::Increment, None),
            "decrement" => (Action::Decrement, None),
            "number" => {
                let Ok(n) = value.parse::<f64>() else {
                    return false;
                };
                if !n.is_finite() {
                    return false;
                }
                (Action::SetValue, Some(ActionData::NumericValue(n)))
            }
            "text" => {
                // egui's text editor consumes keyboard/text events after focus.
                if node.role() != accesskit::Role::TextInput || value.len() > 4096 {
                    return false;
                }
                let modifiers = Modifiers {
                    command: true,
                    ctrl: true,
                    ..Default::default()
                };
                self.events.push(Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                });
                self.events.push(Event::Text(value.to_owned()));
                self.events.push(Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
                return true;
            }
            _ => return false,
        };
        if !node.supports_action(action) {
            return false;
        }
        // egui supports ScrollIntoView on widgets but does not advertise it.
        // Keep keyboard-focused controls visible inside Rust scroll areas.
        if matches!(action, Action::Focus | Action::SetValue) {
            self.events
                .push(Event::AccessKitActionRequest(ActionRequest {
                    action: Action::ScrollIntoView,
                    target: NodeId(id),
                    data: None,
                }));
        }
        self.events
            .push(Event::AccessKitActionRequest(ActionRequest {
                action,
                target: NodeId(id),
                data,
            }));
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_focus_covers_semantic_controls_but_preserves_window_blur() {
        let shared = Arc::new(Mutex::new(Bridge::default()));
        let mut plugin = Plugin(shared.clone());
        let mut input = RawInput {
            focused: false,
            ..Default::default()
        };
        shared.lock().unwrap().browser_focus = Some(true);
        egui::Plugin::input_hook(&mut plugin, &mut input);
        assert!(input.focused);
        shared.lock().unwrap().browser_focus = Some(false);
        egui::Plugin::input_hook(&mut plugin, &mut input);
        assert!(!input.focused);
    }
    #[test]
    fn stale_disabled_and_unadvertised_actions_are_rejected() {
        let mut b = Bridge::default();
        let mut n = accesskit::Node::new(accesskit::Role::Button);
        n.add_action(Action::Click);
        b.nodes.insert(1, n);
        assert!(b.action("1", "click", ""));
        assert!(!b.action("2", "click", ""));
        assert!(!b.action("1", "number", "5"));
        b.nodes.get_mut(&1).unwrap().set_disabled();
        assert!(!b.action("1", "click", ""));
    }
}
