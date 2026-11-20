//! Short rolling average for runtime estimates; raw power measurements stay unchanged.
use crate::battery::{Battery, Status};
use std::collections::VecDeque;

#[derive(Default)]
pub struct RuntimeEstimator {
    context: Option<(String, Status, bool, Option<u8>)>,
    samples: VecDeque<(i64, f64)>,
}

impl RuntimeEstimator {
    pub fn estimate(
        &mut self,
        batteries: &[Battery],
        combined: Option<&Battery>,
        now: i64,
    ) -> Option<f64> {
        let Some(battery) = combined else {
            self.context = None;
            self.samples.clear();
            return None;
        };
        let identity = batteries
            .iter()
            .map(Battery::identity)
            .collect::<Vec<_>>()
            .join("|");
        let context = (
            identity,
            battery.status,
            battery.ac_online,
            battery.end_threshold,
        );
        let gap = self
            .samples
            .back()
            .is_some_and(|(ts, _)| now < *ts || now - ts > 30);
        if self.context.as_ref() != Some(&context) || gap {
            self.samples.clear();
        }
        self.context = Some(context);
        let Some(power) = battery.power_w.filter(|p| p.is_finite() && *p > 0.1) else {
            self.samples.clear();
            return None;
        };
        if !matches!(battery.status, Status::Charging | Status::Discharging) {
            self.samples.clear();
            return None;
        }
        // Do not give repeated refresh clicks extra weight.
        if self.samples.back().is_some_and(|(ts, _)| *ts == now) {
            self.samples.pop_back();
        }
        self.samples.push_back((now, power));
        while self.samples.front().is_some_and(|(ts, _)| now - ts > 120) {
            self.samples.pop_front();
        }
        let watts = self.samples.iter().map(|(_, w)| w).sum::<f64>() / self.samples.len() as f64;
        battery.hours_remaining_at(watts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn battery() -> Battery {
        let dir = std::env::temp_dir().join(format!(
            "bhc-estimator-{}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("test")
                .replace(':', "_")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("energy_full"), "50000000").unwrap();
        std::fs::write(dir.join("energy_now"), "25000000").unwrap();
        std::fs::write(dir.join("status"), "Discharging").unwrap();
        crate::battery::read_battery(&dir).unwrap()
    }
    #[test]
    fn smooths_spikes_and_resets_after_sleep_or_state_changes() {
        let mut b = battery();
        b.power_w = Some(10.0);
        let mut e = RuntimeEstimator::default();
        for t in (0..20).step_by(2) {
            assert_eq!(e.estimate(&[b.clone()], Some(&b), t), Some(2.5));
        }
        b.power_w = Some(40.0);
        let h = e.estimate(&[b.clone()], Some(&b), 20).unwrap();
        assert!(h > 1.8 && h < 2.5);
        assert_eq!(e.estimate(&[b.clone()], Some(&b), 100), Some(0.625));
        b.status = Status::Full;
        assert!(e.estimate(&[b.clone()], Some(&b), 102).is_none());
    }
    #[test]
    fn rejects_invalid_power() {
        let mut b = battery();
        b.power_w = Some(f64::NAN);
        assert!(RuntimeEstimator::default()
            .estimate(&[b.clone()], Some(&b), 0)
            .is_none());
    }
}
