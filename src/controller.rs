//! Periodically samples the battery and apps, records history, and notifies
//! the UI. Lives for the whole application, so history keeps being recorded
//! when the app runs in the background without a window.

use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

use gtk::glib;

use crate::apps::{AppSampler, AppUsage};
use crate::battery::{self, Battery};
use crate::history::{self, History};
use crate::system::PowerProfiles;

const TICK_SECS: u32 = 2;
const APPS_EVERY_TICKS: u64 = 2;
const HISTORY_INTERVAL_SECS: i64 = 60;

pub struct State {
    pub battery: Option<Battery>,
    pub history: History,
    pub apps_now: Vec<AppUsage>,
    pub apps_avg: Vec<AppUsage>,
    /// Incremented whenever the app lists change.
    pub apps_generation: u64,
    /// Incremented whenever a history sample is added.
    pub history_generation: u64,
}

type Listener = Box<dyn Fn(&State) -> glib::ControlFlow>;

pub struct Controller {
    state: RefCell<State>,
    sampler: RefCell<AppSampler>,
    /// Lowest power seen on battery this session; used as the idle floor
    /// when splitting power between apps.
    idle_floor_w: Cell<Option<f64>>,
    ticks: Cell<u64>,
    listeners: RefCell<Vec<Listener>>,
    pub profiles: Option<PowerProfiles>,
}

impl Controller {
    pub fn new() -> Rc<Self> {
        Rc::new(Controller {
            state: RefCell::new(State {
                battery: battery::primary_battery(),
                history: History::load(),
                apps_now: Vec::new(),
                apps_avg: Vec::new(),
                apps_generation: 0,
                history_generation: 0,
            }),
            sampler: RefCell::new(AppSampler::new()),
            idle_floor_w: Cell::new(None),
            ticks: Cell::new(0),
            listeners: RefCell::new(Vec::new()),
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

    /// Calls `f` now and after every update until it returns Break.
    pub fn subscribe(&self, f: impl Fn(&State) -> glib::ControlFlow + 'static) {
        if f(&self.state.borrow()).is_continue() {
            self.listeners.borrow_mut().push(Box::new(f));
        }
    }

    /// Re-reads the battery immediately (e.g. after changing a setting).
    pub fn refresh(&self) {
        self.state.borrow_mut().battery = battery::primary_battery();
        self.notify();
    }

    fn tick(&self) {
        let n = self.ticks.get();
        self.ticks.set(n + 1);
        {
            let mut st = self.state.borrow_mut();
            st.battery = battery::primary_battery();

            if n.is_multiple_of(APPS_EVERY_TICKS) {
                let dynamic = st.battery.as_ref().filter(|b| b.is_on_battery()).and_then(|b| b.power_w).map(|p| {
                    let floor = match self.idle_floor_w.get() {
                        Some(f) if f <= p => f,
                        _ => {
                            self.idle_floor_w.set(Some(p));
                            p
                        }
                    };
                    // Until we've seen a lower reading, assume half is baseline
                    // (display, Wi-Fi, etc.) rather than app-driven.
                    if floor == p { p * 0.5 } else { p - floor }
                });
                let mut sampler = self.sampler.borrow_mut();
                st.apps_now = sampler.sample(dynamic);
                st.apps_avg = sampler.averages();
                st.apps_generation += 1;
            }

            let due = st.history.last_ts().is_none_or(|t| history::now_unix() - t >= HISTORY_INTERVAL_SECS);
            if due {
                if let Some(b) = st.battery.clone() {
                    st.history.record(&b);
                    st.history_generation += 1;
                }
            }
        }
        self.notify();
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
