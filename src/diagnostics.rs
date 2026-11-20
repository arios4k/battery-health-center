//! Read-only power management checks. Never stop or reconfigure competing services.
use crate::battery::Battery;
use crate::i18n::{tr, trf};
use crate::system;
use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib::prelude::ToVariant;

#[derive(Clone)]
pub struct Finding {
    pub warning: bool,
    pub title: String,
    pub detail: String,
}
impl Finding {
    fn new(warning: bool, title: &str, detail: impl Into<String>) -> Self {
        Self {
            warning,
            title: title.into(),
            detail: detail.into(),
        }
    }
}

fn active_managers() -> Option<Vec<String>> {
    let proxy = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
        gio::Cancellable::NONE,
    )
    .ok()?;
    proxy.name_owner()?;
    let mut active = Vec::new();
    for (unit, label) in [
        ("tlp.service", "TLP"),
        ("auto-cpufreq.service", "auto-cpufreq"),
        ("tuned.service", "TuneD"),
    ] {
        let reply = proxy.call_sync(
            "GetUnit",
            Some(&(unit,).to_variant()),
            gio::DBusCallFlags::NONE,
            1500,
            gio::Cancellable::NONE,
        );
        let Ok(reply) = reply else { continue };
        let path = reply.child_value(0).str()?.to_string();
        let unit_proxy = gio::DBusProxy::for_bus_sync(
            gio::BusType::System,
            gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
            None,
            "org.freedesktop.systemd1",
            &path,
            "org.freedesktop.systemd1.Unit",
            gio::Cancellable::NONE,
        )
        .ok()?;
        if unit_proxy
            .cached_property("ActiveState")
            .and_then(|v| v.str().map(String::from))
            .is_some_and(|s| s == "active" || s == "activating")
        {
            active.push(label.into());
        }
    }
    Some(active)
}

pub fn collect(batteries: &[Battery], profiles_available: bool) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.push(Finding::new(!profiles_available, tr("Power Mode Service"), if profiles_available {
        tr("Power profiles are available.")
    } else { tr("Power profiles are unavailable. Check that power-profiles-daemon or a compatible provider is running.") }));
    if system::in_snap() {
        let missing = system::missing_snap_plugs();
        findings.push(Finding::new(
            !missing.is_empty(),
            tr("Snap Permissions"),
            if missing.is_empty() {
                tr("Required battery and power interfaces are connected.").into()
            } else {
                trf(
                    "Connect the missing interfaces, then check again: {interfaces}",
                    &[("interfaces", missing.join(", "))],
                )
            },
        ));
    } else {
        findings.push(Finding::new(system::helper_path().is_none(), tr("Charging Helper"), if system::helper_path().is_some() {
            tr("The charging helper is installed. New charging features require the matching helper version.")
        } else { tr("The charging helper is missing. Install the complete package to change charge limits and schedules.") }));
    }
    match active_managers() {
        Some(active) if active.is_empty() => findings.push(Finding::new(false, tr("Other Power Managers"), tr("No active TLP, auto-cpufreq or TuneD service was detected."))),
        Some(active) => findings.push(Finding::new(true, tr("Other Power Managers"), trf("Active: {managers}. These tools may also change power modes or charge limits. Review their settings if your choices do not stay applied.", &[("managers", active.join(", "))]))),
        None => findings.push(Finding::new(false, tr("Other Power Managers"), tr("Service status could not be inspected. This can happen in a sandbox or on systems without systemd."))),
    }
    for b in batteries {
        if let Some(saved) = system::saved_limit(&b.name) {
            if b.end_threshold.is_some_and(|end| end != saved)
                && !system::full_charge_active(&b.name)
                && !system::scheduled_full_active(&b.name)
            {
                findings.push(Finding::new(true, tr("Charge Limit Changed"), trf("{battery}: saved limit {saved}%, current limit {current}%. Another utility or firmware may have changed it. Review the Charging page before reapplying.",
                    &[("battery", b.name.clone()), ("saved", saved.to_string()), ("current", b.end_threshold.unwrap_or(100).to_string())])));
            }
        }
    }
    if batteries.iter().all(|b| !b.supports_end_threshold()) {
        findings.push(Finding::new(false, tr("Charge Limit Support"), tr("No charge-threshold interface was reported. Monitoring remains available; firmware and driver support determine charge controls.")));
    }
    findings
}
