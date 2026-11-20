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
    pub one_off: Option<i64>,
    pub skip_ready: Option<i64>,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule { optimized: false, lead_min: DEFAULT_LEAD_MIN, rules: Vec::new(), one_off: None, skip_ready: None }
    }
}

impl Schedule {
    /// Nothing to carry out: no rules and no learning.
    pub fn is_empty(&self) -> bool {
        !self.optimized && self.rules.is_empty() && self.one_off.is_none()
    }

    /// Reads the config text, ignoring lines it doesn't understand.
    pub fn parse(text: &str) -> Schedule {
        let mut s = Schedule::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.as_slice() {
                ["once", v] => s.one_off = v.parse::<i64>().ok().filter(|t| *t > 0),
                ["skip", v] => s.skip_ready = v.parse::<i64>().ok().filter(|t| *t > 0),
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
        if let Some(ts) = self.one_off { t.push_str(&format!("once {ts}\n")); }
        if let Some(ts) = self.skip_ready { t.push_str(&format!("skip {ts}\n")); }
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

/// Ready-by occurrences on the local calendar, including the previous day for grace windows.
fn occurrences(schedule: &Schedule, learned: &Learned, now: i64) -> Vec<i64> {
    let mut rules = schedule.rules.clone();
    if schedule.optimized { rules.extend(learned.rules()); }
    let t = now as libc::time_t;
    // SAFETY: localtime_r fills the initialized output; mktime normalizes a copy.
    let local = unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut local).is_null() { return Vec::new(); }
        local
    };
    let weekday = (local.tm_wday + 6) % 7;
    let mut result: Vec<i64> = (-1..=7).flat_map(|days| {
        let rules = &rules;
        rules.iter().filter_map(move |rule| {
            if rule.days >> (weekday + days).rem_euclid(7) & 1 == 0 { return None; }
            let mut ready = local;
            ready.tm_mday += days;
            ready.tm_hour = i32::from(rule.ready / 60);
            ready.tm_min = i32::from(rule.ready % 60);
            ready.tm_sec = 0;
            ready.tm_isdst = -1;
            // SAFETY: ready is initialized; a negative result denotes conversion failure here.
            let ts = unsafe { libc::mktime(&mut ready) } as i64;
            (ts > 0).then_some(ts)
        })
    }).collect();
    result.extend(schedule.one_off);
    result.sort_unstable(); result.dedup();
    result
}

/// Next unskipped ready-by timestamp, interpreted in the system's local timezone.
pub fn next_ready(schedule: &Schedule, learned: &Learned, now: i64) -> Option<i64> {
    occurrences(schedule, learned, now).into_iter().filter(|ts| *ts > now && Some(*ts) != schedule.skip_ready).min()
}

pub fn full_charge_wanted_at(schedule: &Schedule, learned: &Learned, now: i64) -> bool {
    occurrences(schedule, learned, now).into_iter().any(|ts| Some(ts) != schedule.skip_ready
        && now >= ts - i64::from(schedule.lead_min) * 60 && now <= ts + i64::from(GRACE_MIN) * 60)
}

impl Schedule {
    pub fn set_one_off(&mut self, ready: Option<i64>, now: i64) -> Result<(), String> {
        if ready.is_some_and(|ts| ts <= now + 60 || ts > now + 366 * 86_400) {
            return Err("Choose a ready-by time at least one minute ahead and within the next year.".into());
        }
        if self.skip_ready == self.one_off { self.skip_ready = None; }
        self.one_off = ready; Ok(())
    }

    pub fn skip_next(&mut self, learned: &Learned, now: i64) -> Result<(), String> {
        if self.skip_ready.is_some_and(|ts| ts + i64::from(GRACE_MIN) * 60 >= now) {
            return Err("A charge is already skipped. Undo it before skipping another.".into());
        }
        self.skip_ready = Some(next_ready(self, learned, now).ok_or("There is no upcoming charge to skip.")?);
        Ok(())
    }

    pub fn expire(&mut self, now: i64) -> bool {
        let before = (self.one_off, self.skip_ready);
        if self.one_off.is_some_and(|ts| ts + i64::from(GRACE_MIN) * 60 < now) { self.one_off = None; }
        if self.skip_ready.is_some_and(|ts| ts + i64::from(GRACE_MIN) * 60 < now) { self.skip_ready = None; }
        before != (self.one_off, self.skip_ready)
    }

    /// Typed local calendar input. Reject invalid and DST-normalized nonexistent times.
    pub fn local_ready(date: &str, time: &str) -> Option<i64> {
        let parts: Vec<_> = date.split('-').collect();
        if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 { return None; }
        let (year, month, day) = (parts[0].parse::<i32>().ok()?, parts[1].parse::<i32>().ok()?, parts[2].parse::<i32>().ok()?);
        let minute = parse_time(time)?;
        if !(1970..=2200).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) { return None; }
        // SAFETY: mktime initializes the calendar normalization of this local tm.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_year = year - 1900; tm.tm_mon = month - 1; tm.tm_mday = day;
        tm.tm_hour = i32::from(minute / 60); tm.tm_min = i32::from(minute % 60); tm.tm_isdst = -1;
        let ts = unsafe { libc::mktime(&mut tm) } as i64;
        (ts > 0 && tm.tm_year == year - 1900 && tm.tm_mon == month - 1 && tm.tm_mday == day
            && tm.tm_hour == i32::from(minute / 60) && tm.tm_min == i32::from(minute % 60)).then_some(ts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(weekday: u8, h: u16, m: u16) -> Clock {
        Clock { weekday, minute: h * 60 + m }
    }

    #[test]
    fn next_ready_is_future_and_respects_enabled_learned_rules() {
        let now = 1_800_000_000;
        let clock = clock_at(now).unwrap();
        let next_minute = (clock.minute + 1) % 1440;
        let plan = Schedule { rules: vec![Rule { days: EVERY_DAY, ready: next_minute }], ..Schedule::default() };
        let ts = next_ready(&plan, &Learned::default(), now).unwrap();
        assert!(ts > now && ts - now <= 60);
        assert_eq!(clock_at(ts).unwrap().minute, next_minute);
        assert!(next_ready(&Schedule::default(), &Learned { weekdays: Some(next_minute), weekend: Some(next_minute), ..Learned::default() }, now).is_none());
        let learned = Learned { weekdays: Some(next_minute), weekend: Some(next_minute), ..Learned::default() };
        assert_eq!(next_ready(&Schedule { optimized: true, ..Schedule::default() }, &learned, now), Some(ts));
        let week = Schedule { rules: vec![Rule { days: 1 << clock.weekday, ready: clock.minute }], ..Schedule::default() };
        let upcoming = next_ready(&week, &Learned::default(), now).unwrap();
        assert!(upcoming - now > 6 * 86_400);
    }

    #[test]
    fn one_off_can_be_skipped_restored_and_expires_without_losing_rules() {
        let now = 1_800_000_000;
        let mut plan = Schedule::default();
        plan.set_one_off(Some(now + 3600), now).unwrap();
        assert!(full_charge_wanted_at(&plan, &Learned::default(), now));
        assert_eq!(Schedule::parse(&plan.to_text()), plan);
        plan.skip_next(&Learned::default(), now).unwrap();
        assert!(!full_charge_wanted_at(&plan, &Learned::default(), now));
        assert!(next_ready(&plan, &Learned::default(), now).is_none());
        assert!(plan.skip_next(&Learned::default(), now).is_err());
        plan.skip_ready = None;
        assert!(full_charge_wanted_at(&plan, &Learned::default(), now));
        plan.rules.push(Rule { days: EVERY_DAY, ready: 480 });
        assert!(plan.expire(now + 3600 + i64::from(GRACE_MIN) * 60 + 1));
        assert!(plan.one_off.is_none()); assert_eq!(plan.rules.len(), 1);
        assert!(plan.set_one_off(Some(now - 1), now).is_err());
    }

    #[test]
    fn typed_local_dates_reject_invalid_calendar_values() {
        assert!(Schedule::local_ready("2026-02-30", "08:00").is_none());
        assert!(Schedule::local_ready("2026-11-20", "25:00").is_none());
        assert!(Schedule::local_ready("2026-1-1", "08:00").is_none());
        let ts = Schedule::local_ready("2026-11-20", "08:30").unwrap();
        assert_eq!(clock_at(ts).unwrap().minute, 510);
    }

    #[test]
    fn text_roundtrip_and_clamping() {
        let s = Schedule { optimized: true, lead_min: 90, rules: vec![Rule { days: WEEKDAYS, ready: 7 * 60 + 30 }, Rule { days: 64, ready: 600 }], ..Schedule::default() };
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
