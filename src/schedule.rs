//! The charging schedule: when to charge to 100% so the battery is full just
//! before you need the laptop, and a guess at when that is from the times you
//! usually unplug. Shared by the app (which edits and shows it) and the root
//! helper (which carries it out); it only reads and writes text.

/// Minutes after the ready-by time during which the limit stays lifted, for
/// when you leave a little late.
pub const GRACE_MIN: i32 = 30;
pub const MIN_LEAD_MIN: u16 = 30;
pub const MAX_LEAD_MIN: u16 = 360;
pub const DEFAULT_LEAD_MIN: u16 = 120;
pub const MAX_RULES: usize = 14;
/// Unplug times needed before a pattern counts as learned.
pub const MIN_SAMPLES: usize = 3;
/// A pattern is learned only if the middle half of the unplug times falls
/// within this many minutes.
pub const MAX_SPREAD_MIN: u16 = 90;
/// Only sessions plugged in at least this long count, so quickly moving the
/// charger doesn't teach a pattern.
pub const MIN_SESSION_SECS: i64 = 3 * 3600;
/// Learning looks at this many days of unplug times.
pub const LEARN_DAYS: i64 = 28;

/// Days as a bit mask: Monday is bit 0 … Sunday is bit 6.
pub const WEEKDAYS: u8 = 0b001_1111;
pub const WEEKEND: u8 = 0b110_0000;
pub const EVERY_DAY: u8 = 0b111_1111;

/// "Be at 100% by this time on these days."
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub days: u8,
    /// Minutes since midnight, local time.
    pub ready: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// Also use the times learned from when you unplug.
    pub optimized: bool,
    /// How long before the ready-by time to start charging to 100%.
    pub lead_min: u16,
    pub rules: Vec<Rule>,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule { optimized: false, lead_min: DEFAULT_LEAD_MIN, rules: Vec::new() }
    }
}

impl Schedule {
    /// Nothing to carry out: no rules and no learning.
    pub fn is_empty(&self) -> bool {
        !self.optimized && self.rules.is_empty()
    }

    /// Reads the config text, ignoring lines it doesn't understand.
    pub fn parse(text: &str) -> Schedule {
        let mut s = Schedule::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.as_slice() {
                ["optimized", v] => s.optimized = *v == "1",
                ["lead", v] => {
                    if let Ok(m) = v.parse::<u16>() {
                        s.lead_min = m.clamp(MIN_LEAD_MIN, MAX_LEAD_MIN);
                    }
                }
                ["rule", days, time] => {
                    if let (Ok(days), Some(ready)) = (days.parse::<u8>(), parse_time(time)) {
                        if days & EVERY_DAY != 0 && days <= EVERY_DAY && s.rules.len() < MAX_RULES {
                            s.rules.push(Rule { days, ready });
                        }
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let mut t = String::from("# Managed by Battery Health Center: optimized, lead (minutes), rule <days> <HH:MM>\n");
        t.push_str(&format!("optimized {}\nlead {}\n", u8::from(self.optimized), self.lead_min));
        for r in &self.rules {
            t.push_str(&format!("rule {} {}\n", r.days, format_time(r.ready)));
        }
        t
    }
}

/// "07:30" → 450.
pub fn parse_time(s: &str) -> Option<u16> {
    let (h, m) = s.split_once(':')?;
    let (h, m) = (h.parse::<u16>().ok()?, m.parse::<u16>().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn format_time(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// A moment on the local clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    /// Monday is 0 … Sunday is 6.
    pub weekday: u8,
    pub minute: u16,
}

/// The local weekday and time of a Unix timestamp.
pub fn clock_at(ts: i64) -> Option<Clock> {
    let t = ts as libc::time_t;
    // SAFETY: `tm` is plain data that localtime_r fills in.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return None;
        }
        tm
    };
    Some(Clock { weekday: ((tm.tm_wday + 6) % 7) as u8, minute: (tm.tm_hour * 60 + tm.tm_min) as u16 })
}

/// When you usually unplug, learned from past unplug times.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learned {
    /// Minutes since midnight, or None while there isn't a clear pattern.
    pub weekdays: Option<u16>,
    pub weekend: Option<u16>,
    pub weekday_samples: usize,
    pub weekend_samples: usize,
}

impl Learned {
    /// The learned times as rules.
    pub fn rules(&self) -> Vec<Rule> {
        let mut rules = Vec::new();
        if let Some(ready) = self.weekdays {
            rules.push(Rule { days: WEEKDAYS, ready });
        }
        if let Some(ready) = self.weekend {
            rules.push(Rule { days: WEEKEND, ready });
        }
        rules
    }
}

/// Learns from the local weekday and time of each unplug.
pub fn learn_from(unplugs: &[Clock]) -> Learned {
    let group = |weekend: bool| -> (Option<u16>, usize) {
        let mut minutes: Vec<u16> = unplugs.iter().filter(|c| (c.weekday >= 5) == weekend).map(|c| c.minute).collect();
        minutes.sort_unstable();
        if minutes.len() < MIN_SAMPLES {
            return (None, minutes.len());
        }
        let (q1, q3) = (minutes[minutes.len() / 4], minutes[minutes.len() * 3 / 4]);
        // The early end of the usual range, so the battery is full even on an
        // early day.
        ((q3 - q1 <= MAX_SPREAD_MIN).then_some(q1), minutes.len())
    };
    let (weekdays, weekday_samples) = group(false);
    let (weekend, weekend_samples) = group(true);
    Learned { weekdays, weekend, weekday_samples, weekend_samples }
}

/// Learns from unplug timestamps, using only the recent ones.
pub fn learn(unplugs: &[i64], now_ts: i64) -> Learned {
    let clocks: Vec<Clock> =
        unplugs.iter().filter(|ts| now_ts - **ts <= LEARN_DAYS * 86_400).filter_map(|ts| clock_at(*ts)).collect();
    learn_from(&clocks)
}

/// Whether the limit should be lifted now: some ready-by time (yours, or
/// learned when optimized charging is on) is within the lead time ahead, or
/// just passed.
pub fn full_charge_wanted(schedule: &Schedule, learned: &Learned, now: Clock) -> bool {
    let mut rules = schedule.rules.clone();
    if schedule.optimized {
        rules.extend(learned.rules());
    }
    let lead = i32::from(schedule.lead_min);
    rules.iter().any(|r| {
        // Today, and the neighbouring days for windows that cross midnight.
        (-1..=1).any(|d: i32| {
            let weekday = (i32::from(now.weekday) + d).rem_euclid(7);
            if r.days >> weekday & 1 == 0 {
                return false;
            }
            let until_ready = d * 1440 + i32::from(r.ready) - i32::from(now.minute);
            (-GRACE_MIN..=lead).contains(&until_ready)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(weekday: u8, h: u16, m: u16) -> Clock {
        Clock { weekday, minute: h * 60 + m }
    }

    #[test]
    fn text_roundtrip_and_clamping() {
        let s = Schedule { optimized: true, lead_min: 90, rules: vec![Rule { days: WEEKDAYS, ready: 7 * 60 + 30 }, Rule { days: 64, ready: 600 }] };
        assert_eq!(Schedule::parse(&s.to_text()), s);
        let odd = Schedule::parse("lead 5\nrule 0 07:00\nrule 200 07:00\nrule 3 25:00\nrule 3 07:61\nnonsense\nrule 3 06:15\n");
        assert_eq!(odd.lead_min, MIN_LEAD_MIN);
        assert_eq!(odd.rules, vec![Rule { days: 3, ready: 6 * 60 + 15 }]);
        assert!(Schedule::default().is_empty());
    }

    #[test]
    fn rules_are_capped() {
        let text: String = (0..30).map(|_| "rule 1 07:00\n").collect();
        assert_eq!(Schedule::parse(&text).rules.len(), MAX_RULES);
    }

    #[test]
    fn window_before_and_after_the_ready_time() {
        let s = Schedule { rules: vec![Rule { days: WEEKDAYS, ready: 8 * 60 }], ..Schedule::default() }; // lead 120
        let none = Learned::default();
        assert!(!full_charge_wanted(&s, &none, at(0, 5, 0))); // too early
        assert!(full_charge_wanted(&s, &none, at(0, 6, 0))); // lead starts
        assert!(full_charge_wanted(&s, &none, at(0, 8, 0)));
        assert!(full_charge_wanted(&s, &none, at(0, 8, 30))); // grace
        assert!(!full_charge_wanted(&s, &none, at(0, 8, 31)));
        assert!(!full_charge_wanted(&s, &none, at(5, 7, 0))); // Saturday isn't a weekday
    }

    #[test]
    fn window_can_start_the_evening_before() {
        let s = Schedule { lead_min: 180, rules: vec![Rule { days: 1, ready: 60 }], ..Schedule::default() }; // Mon 01:00
        let none = Learned::default();
        assert!(full_charge_wanted(&s, &none, at(6, 22, 30))); // Sunday 22:30, 2.5 h ahead
        assert!(!full_charge_wanted(&s, &none, at(6, 21, 30)));
    }

    #[test]
    fn learns_a_steady_pattern_and_ignores_a_scattered_one() {
        let steady: Vec<Clock> = [(0, 8, 10), (1, 8, 20), (2, 8, 0), (3, 8, 40), (4, 8, 15)].iter().map(|&(d, h, m)| at(d, h, m)).collect();
        let l = learn_from(&steady);
        assert_eq!(l.weekdays, Some(8 * 60 + 10)); // the early end of the usual range
        assert_eq!(l.weekend, None);
        assert_eq!((l.weekday_samples, l.weekend_samples), (5, 0));
        let scattered: Vec<Clock> = [(0, 6, 0), (1, 12, 0), (2, 18, 0), (3, 9, 0)].iter().map(|&(d, h, m)| at(d, h, m)).collect();
        assert_eq!(learn_from(&scattered).weekdays, None);
        assert_eq!(learn_from(&steady[..2]).weekdays, None); // too few
    }

    #[test]
    fn learned_times_only_count_when_optimized_is_on() {
        let learned = Learned { weekdays: Some(8 * 60), ..Learned::default() };
        let mut s = Schedule::default();
        assert!(!full_charge_wanted(&s, &learned, at(1, 7, 0)));
        s.optimized = true;
        assert!(full_charge_wanted(&s, &learned, at(1, 7, 0)));
    }
}
