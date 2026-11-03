//! Shared adaptive workspace: two columns on desktop, one on smaller windows.
use adw::prelude::*;

pub struct Workspace {
    pub root: adw::BreakpointBin,
    primary: gtk::Box,
    secondary: gtk::Box,
    body: gtk::Box,
}

impl Workspace {
    pub fn new() -> Self { Self::build(false) }

    pub fn single_column() -> Self { Self::build(true) }

    fn build(single: bool) -> Self {
        let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18)
            .css_classes(["workspace-body"]).build();
        let columns = gtk::Box::builder().orientation(if single { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal }).spacing(20).homogeneous(!single).valign(gtk::Align::Start).build();
        let column = || gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18)
            .hexpand(true).valign(gtk::Align::Start).build();
        let primary = column();
        let secondary = column();
        secondary.set_visible(false);
        columns.append(&primary);
        columns.append(&secondary);
        body.append(&columns);
        let clamp = adw::Clamp::builder().maximum_size(if single { 800 } else { 1400 })
            .tightening_threshold(if single { 600 } else { 1100 }).child(&body).build();
        let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&clamp).build();
        let root = adw::BreakpointBin::builder().child(&scroll).width_request(280).height_request(240).build();
        let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 820sp").unwrap());
        narrow.add_setter(&columns, "orientation", Some(&gtk::Orientation::Vertical.to_value()));
        narrow.add_setter(&columns, "homogeneous", Some(&false.to_value()));
        root.add_breakpoint(narrow);
        Self { root, primary, secondary, body }
    }

    pub fn add(&self, widget: &impl IsA<gtk::Widget>) { self.primary.append(widget); }
    pub fn aside(&self, widget: &impl IsA<gtk::Widget>) {
        self.secondary.append(widget);
        let secondary = self.secondary.downgrade();
        let update = move || {
            if let Some(secondary) = secondary.upgrade() {
                let mut child = secondary.first_child();
                let mut visible = false;
                while let Some(widget) = child {
                    visible |= widget.get_visible();
                    child = widget.next_sibling();
                }
                secondary.set_visible(visible);
            }
        };
        update();
        widget.as_ref().connect_visible_notify(move |_| update());
    }
    pub fn banner(&self, widget: &impl IsA<gtk::Widget>) { self.body.prepend(widget); }
}
