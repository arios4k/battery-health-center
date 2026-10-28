//! Periodically samples the battery and apps, records history, and notifies
//! the UI. Lives for the whole application, so history keeps being recorded
//! when the app runs in the background without a window.
//!
//! Each launch is its own process (the window and the hidden background copy),
//! so only the process that writes history (see `History::is_writer`) keeps the other logs, sends
//! notifications and advances calibration; the others just display.

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Instant, SystemTime};

use gtk::glib;

use crate::app_history::AppHistory;
use crate::apps::{AppSampler, AppUsage};
use crate::battery::{self, Battery, Charger, Status};
use crate::health_log::HealthLog;
use crate::history::{self, History};
use crate::settings::{Calibration, Settings};
use crate::system::{self, DeviceBattery, PowerProfiles};
use crate::i18n::{tr, trf};

const TICK_SECS: u32 = 2;
const APPS_EVERY_TICKS: u64 = 2;
const DEVICES_EVERY_TICKS: u64 = 15;
const SETTINGS_EVERY_TICKS: u64 = 5;
const HISTORY_INTERVAL_SECS: i64 = 60;

/// An app this busy for this long on battery triggers a notification.
const APP_DRAIN_IMPACT: f64 = 70.0;
const APP_DRAIN_SECS: i64 = 120;
/// Sleep drain above this (percent per hour) triggers a notification.
pub const HIGH_SLEEP_DRAIN: f64 = 2.0;
/// Calibration discharges down to this level.
pub const CALIBRATION_EMPTY: f64 = 5.0;

pub struct State {
    /// Overall view (combined when there are several batteries).
    pub battery: Option<Battery>,
    pub batteries: Vec<Battery>,
    pub charger: Option<Charger>,
    /// Selected and available system sleep modes.
    pub sleep_mode: Option<(String, Vec<String>)>,
    pub devices: Vec<DeviceBattery>,
    /// A one-time charge to 100% is in progress.
    pub full_charge: bool,
    pub history: History,
    pub health_log: HealthLog,
    pub app_history: AppHistory,
    pub apps_now: Vec<AppUsage>,
    pub apps_avg: Vec<AppUsage>,
    pub settings: Settings,
    /// Incremented whenever the app lists change.
    pub apps_generation: u64,
    /// Incremented whenever a history sample is added.
    pub history_generation: u64,
}

type Listener = Box<dyn Fn(&State) -> glib::ControlFlow>;
type Notifier = Box<dyn Fn(&str, &str, &str)>;

/// What has already been announced, so each notification is sent once.
#[derive(Default)]
struct Announced {
    limit_reached: bool,
    low: bool,
    full_no_limit: bool,
    /// App key -> when it became busy, and when it was last announced.
    app_busy_since: HashMap<String, i64>,
    app_announced: HashMap<String, i64>,
    last_sleep_ts: i64,
}

pub struct Controller {
    state: RefCell<State>,
    sampler: RefCell<AppSampler>,
    /// Lowest power seen on battery this session; used as the idle floor
    /// when splitting power between apps.
    idle_floor_w: Cell<Option<f64>>,
    last_apps_sample: Cell<Option<Instant>>,
    ticks: Cell<u64>,
    listeners: RefCell<Vec<Listener>>,
    notifier: RefCell<Option<Notifier>>,
    announced: RefCell<Announced>,
    settings_mtime: Cell<Option<SystemTime>>,
    /// A helper request started by the controller is in flight.
    helper_busy: Cell<bool>,
    /// The power mode automatic mode last chose, so it only acts when the
    /// choice changes and never fights a mode picked by hand.
    last_auto_profile: RefCell<Option<String>>,
    pub profiles: Option<PowerProfiles>,
}

fn settings_mtime() -> Option<SystemTime> {
    let path = gtk::glib::user_config_dir().join("battery-health-center").join("settings.ini");
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Controller {
    pub fn new() -> Rc<Self> {
        let batteries = battery::all_batteries();
        let now = history::now_unix();
        Rc::new(Controller {
            state: RefCell::new(State {
                battery: battery::combine(&batteries),
                batteries,
                charger: battery::charger(),
                sleep_mode: battery::sleep_mode(),
                devices: Vec::new(),
                full_charge: false,
                history: History::load(),
                health_log: HealthLog::load(),
                app_history: AppHistory::load(now),
                apps_now: Vec::new(),
                apps_avg: Vec::new(),
                settings: Settings::load(),
                apps_generation: 0,
                history_generation: 0,
            }),
            sampler: RefCell::new(AppSampler::new()),
            idle_floor_w: Cell::new(None),
            last_apps_sample: Cell::new(None),
            ticks: Cell::new(0),
            listeners: RefCell::new(Vec::new()),
            notifier: RefCell::new(None),
            announced: RefCell::new(Announced::default()),
            settings_mtime: Cell::new(settings_mtime()),
            helper_busy: Cell::new(false),
            last_auto_profile: RefCell::new(None),
            profiles: PowerProfiles::connect(),
        })
    }

    pub fn start(self: &Rc<Self>) {
        self.tick();
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(TICK_SECS, move || match weak.upgrade() {
            Some(c) => {
                c.tick();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
    }

    pub fn state(&self) -> Ref<'_, State> {
        self.state.borrow()
    }

    /// Sets how notifications are shown: (id, title, body).
    pub fn set_notifier(&self, f: impl Fn(&str, &str, &str) + 'static) {
        *self.notifier.borrow_mut() = Some(Box::new(f));
    }

    /// Calls `f` now and after every update until it returns Break.
    pub fn subscribe(&self, f: impl Fn(&State) -> glib::ControlFlow + 'static) {
        if f(&self.state.borrow()).is_continue() {
            self.listeners.borrow_mut().push(Box::new(f));
        }
    }

    /// Changes settings, saves them, and updates the UI.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        {
            let mut st = self.state.borrow_mut();
            let before = st.settings.clone();
            f(&mut st.settings);
            if st.settings == before {
                return;
            }
            st.settings.save();
        }
        self.settings_mtime.set(settings_mtime());
        self.notify();
    }

    /// The icon for a stored app key.
    pub fn app_icon(&self, key: &str) -> Option<gtk::gio::Icon> {
        self.sampler.borrow().icon_for_key(key)
    }

    /// Re-reads the battery immediately (e.g. after changing a setting).
    pub fn refresh(&self) {
        self.read_power();
        self.notify();
    }

    fn read_power(&self) {
        let mut st = self.state.borrow_mut();
        st.batteries = battery::all_batteries();
        st.battery = battery::combine(&st.batteries);
        st.charger = battery::charger();
        st.full_charge = st.batteries.iter().any(|b| system::full_charge_active(&b.name));
    }

    fn tick(self: &Rc<Self>) {
        let n = self.ticks.get();
        self.ticks.set(n + 1);
        self.read_power();
        let now = history::now_unix();
        {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let writer = st.history.is_writer();

            if n.is_multiple_of(APPS_EVERY_TICKS) {
                let dynamic = st.battery.as_ref().filter(|b| b.is_on_battery()).and_then(|b| b.power_w).map(|p| {
                    let lowest = self.idle_floor_w.get().map_or(p, |f| f.min(p));
                    self.idle_floor_w.set(Some(lowest));
                    // The baseline (display, Wi-Fi, etc.) is the lowest draw seen,
                    // but never more than half the current draw: early in a session
                    // the lowest reading is usually not a true idle reading.
                    p - lowest.min(p * 0.5)
                });
                let mut sampler = self.sampler.borrow_mut();
                st.apps_now = sampler.sample(dynamic);
                st.apps_avg = sampler.averages();
                let elapsed = self.last_apps_sample.replace(Some(Instant::now())).map(|t| t.elapsed().as_secs_f64());
                // Skip long gaps (e.g. after resuming from sleep).
                if let Some(secs) = elapsed.filter(|s| *s < 30.0) {
                    st.app_history.add(&st.apps_now, secs, now, writer);
                }
                st.apps_generation += 1;
            }

            if n.is_multiple_of(DEVICES_EVERY_TICKS) {
                st.devices = system::device_batteries();
            }

            if n.is_multiple_of(SETTINGS_EVERY_TICKS) {
                // Another process (e.g. an open window) may have changed them.
                let mtime = settings_mtime();
                if mtime != self.settings_mtime.get() {
                    self.settings_mtime.set(mtime);
                    st.settings = Settings::load();
                }
            }

            let due = st.history.last_ts().is_none_or(|t| now - t >= HISTORY_INTERVAL_SECS);
            if due {
                if let Some(b) = st.battery.clone() {
                    st.history.record(&b);
                    st.history_generation += 1;
                }
            }
            if st.history.is_writer() {
                if let Some(b) = st.batteries.first() {
                    st.health_log.record(b, now);
                }
            }
        }
        if self.state.borrow().history.is_writer() {
            self.finish_full_charge_when_done();
            self.advance_calibration();
            self.apply_auto_profile();
            self.send_notifications(n == 0, now);
        }
        self.notify();
    }

    /// Sends helper requests in order without blocking; one batch at a time.
    fn helper(self: &Rc<Self>, requests: Vec<Vec<String>>) {
        if requests.is_empty() || self.helper_busy.replace(true) {
            return;
        }
        let this = self.clone();
        glib::spawn_future_local(async move {
            for args in requests {
                if let Err(e) = system::run_helper(args).await {
                    eprintln!("battery-health-center: {e}");
                }
            }
            this.helper_busy.set(false);
            this.refresh();
        });
    }

    /// Starts a one-time charge to 100% on every battery with a limit.
    pub fn start_full_charge(self: &Rc<Self>) {
        let names: Vec<String> =
            self.state.borrow().batteries.iter().filter(|b| b.supports_end_threshold()).map(|b| b.name.clone()).collect();
        self.helper(names.into_iter().map(|n| vec!["charge-full".into(), n]).collect());
    }

    /// Ends a one-time charge to 100% and puts the limit back.
    pub fn finish_full_charge(self: &Rc<Self>) {
        let names: Vec<String> = self.state.borrow().batteries.iter().map(|b| b.name.clone()).collect();
        self.helper(names.into_iter().map(|n| vec!["finish-full-charge".into(), n]).collect());
    }

    /// The helper only runs when asked, so the app asks it to put the limit
    /// back once a one-time full charge is done.
    fn finish_full_charge_when_done(self: &Rc<Self>) {
        if self.helper_busy.get() {
            return;
        }
        let done = {
            let st = self.state.borrow();
            st.full_charge
                && st.battery.as_ref().is_some_and(|b| b.status == Status::Full || b.percent >= 99.0 || !b.ac_online)
        };
        if done {
            self.finish_full_charge();
        }
    }

    /// Applies "automatic power mode": when the charger is connected or
    /// removed, or the charge crosses the low level, switch to the chosen mode.
    fn apply_auto_profile(&self) {
        let Some(profiles) = self.profiles.as_ref() else { return };
        let wanted = {
            let st = self.state.borrow();
            let Some(b) = st.battery.as_ref() else { return };
            st.settings.desired_profile(b.ac_online, b.percent, &profiles.available())
        };
        let mut last = self.last_auto_profile.borrow_mut();
        if *last == wanted {
            return;
        }
        if let Some(mode) = &wanted {
            if profiles.active().as_deref() != Some(mode.as_str()) {
                if let Err(e) = profiles.set_active(mode) {
                    eprintln!("battery-health-center: could not switch power mode to {mode}: {}", e.message());
                }
            }
        }
        *last = wanted;
    }

    pub fn start_calibration(self: &Rc<Self>) {
        self.update_settings(|s| s.calibration = Calibration::ChargeFull);
        self.start_full_charge();
    }

    pub fn cancel_calibration(self: &Rc<Self>) {
        self.update_settings(|s| s.calibration = Calibration::Off);
        if self.state.borrow().full_charge {
            self.finish_full_charge();
        }
    }

    /// Moves the guided calibration on when each step's condition is met.
    fn advance_calibration(self: &Rc<Self>) {
        let (step, b) = {
            let st = self.state.borrow();
            let Some(b) = st.battery.clone() else { return };
            (st.settings.calibration, b)
        };
        let full = b.status == Status::Full || b.percent >= 99.0;
        match step {
            Calibration::ChargeFull if full && b.ac_online => {
                self.update_settings(|s| s.calibration = Calibration::Discharge);
                self.send("calibration", tr("Calibration: Unplug Your Laptop"), tr("The battery is full. Unplug the charger and use your laptop normally until the battery is nearly empty."));
            }
            Calibration::Discharge if b.is_on_battery() && b.percent <= CALIBRATION_EMPTY => {
                self.update_settings(|s| s.calibration = Calibration::Recharge);
                self.start_full_charge();
                self.send("calibration", tr("Calibration: Plug In Now"), tr("The battery is nearly empty. Connect the charger and let it charge to 100%."));
            }
            Calibration::Recharge if full && b.ac_online => {
                self.update_settings(|s| s.calibration = Calibration::Off);
                self.send("calibration", tr("Calibration Complete"), tr("The battery's capacity reading has been recalibrated. Your charge limit is back on."));
            }
            _ => {}
        }
    }

    fn send(&self, id: &str, title: &str, body: &str) {
        if let Some(f) = self.notifier.borrow().as_ref() {
            f(id, title, body);
        }
    }

    fn send_notifications(&self, first_tick: bool, now: i64) {
        let st = self.state.borrow();
        let Some(b) = st.battery.as_ref() else { return };
        let s = &st.settings;
        let mut a = self.announced.borrow_mut();
        // On the first tick, only note what's already true, so starting the
        // app doesn't announce old news.
        let mut out: Vec<(&str, String, String)> = Vec::new();
        let calibrating = s.calibration != Calibration::Off;

        let limit = b.end_threshold.filter(|e| *e < 100 && b.supports_end_threshold());
        let at_limit = b.ac_online
            && !st.full_charge
            && limit.is_some_and(|l| b.status == Status::NotCharging || (b.status != Status::Charging && b.percent + 1.0 >= l as f64));
        if at_limit && !a.limit_reached && s.notify_limit_reached && !calibrating {
            out.push(("limit", trf("Charged to {0}%", &[("0", limit.unwrap_or(100).to_string())]), tr("Charging has stopped at your limit to protect the battery.").into()));
        }
        a.limit_reached = at_limit || (a.limit_reached && b.ac_online);

        let full_no_limit = b.ac_online
            && !st.full_charge
            && !calibrating
            && b.supports_end_threshold()
            && b.end_threshold.unwrap_or(100) >= 100
            && (b.status == Status::Full || b.percent >= 99.0);
        if full_no_limit && !a.full_no_limit && s.notify_full_no_limit {
            out.push(("full", tr("Battery Fully Charged").into(), tr("Unplug the charger, or set a charge limit in Battery Health Center to slow battery wear.").into()));
        }
        a.full_no_limit = full_no_limit || (a.full_no_limit && b.ac_online);

        let low = b.is_on_battery() && b.percent <= s.low_level as f64;
        if low && !a.low && s.notify_low && !calibrating {
            let remaining = b.hours_remaining().map(|h| format!(" {}", trf("About {0} remaining.", &[("0", battery::format_hours(h).to_string())]))).unwrap_or_default();
            out.push(("low", trf("Battery at {0}%", &[("0", format!("{:.0}", b.percent))]), trf("Connect the charger soon.{remaining}", &[("remaining", remaining.to_string())])));
        }
        a.low = low || (a.low && b.is_on_battery());

        if b.is_on_battery() && s.notify_app_drain {
            let busy: Vec<&AppUsage> = st.apps_now.iter().filter(|x| !x.system && x.impact >= APP_DRAIN_IMPACT).collect();
            a.app_busy_since.retain(|k, _| busy.iter().any(|x| &x.key == k));
            for app in busy {
                let since = *a.app_busy_since.entry(app.key.clone()).or_insert(now);
                let recently = a.app_announced.get(&app.key).is_some_and(|t| now - t < 3600);
                if now - since >= APP_DRAIN_SECS && !recently {
                    a.app_announced.insert(app.key.clone(), now);
                    out.push(("app", trf("{0} Is Using Significant Energy", &[("0", app.name.to_string())]), tr("Quit it if you're not using it to make your battery last longer.").into()));
                }
            }
        } else {
            a.app_busy_since.clear();
        }

        if let Some(sleep) = st.history.sleep_sessions().into_iter().next() {
            if sleep.start_ts > a.last_sleep_ts {
                a.last_sleep_ts = sleep.start_ts;
                let rate = sleep.percent_per_hour();
                if rate >= HIGH_SLEEP_DRAIN && s.notify_sleep_drain {
                    out.push((
                        "sleep",
                        tr("High Battery Drain During Sleep").into(),
                        trf("Your laptop used {0}% in {1} asleep ({rate}% per hour).", &[("0", format!("{:.0}", sleep.from_percent - sleep.to_percent)), ("1", battery::format_hours(sleep.slept_secs as f64 / 3600.0).to_string()), ("rate", format!("{rate:.1}"))]),
                    ));
                }
            }
        }

        drop(a);
        drop(st);
        if !first_tick {
            for (id, title, body) in out {
                self.send(id, &title, &body);
            }
        }
    }

    fn notify(&self) {
        let listeners = std::mem::take(&mut *self.listeners.borrow_mut());
        let st = self.state.borrow();
        let mut kept: Vec<Listener> = listeners.into_iter().filter(|f| f(&st).is_continue()).collect();
        let mut current = self.listeners.borrow_mut();
        kept.append(&mut current);
        *current = kept;
    }
}
