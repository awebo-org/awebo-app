use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    Ask,
    Agent,
    Sandbox,
    Git,
    EditorTabs,
}

impl Feature {
    pub fn label(&self) -> &'static str {
        match self {
            Feature::Ask => "/ask",
            Feature::Agent => "/agent",
            Feature::Sandbox => "sandbox",
            Feature::Git => "git ops",
            Feature::EditorTabs => "editor tabs",
        }
    }

    pub fn free_limit(&self) -> u32 {
        match self {
            Feature::Ask => 20,
            Feature::Agent => 5,
            Feature::Sandbox => 3,
            Feature::Git => 10,
            Feature::EditorTabs => 2,
        }
    }

    pub fn grace_allowance(&self) -> u32 {
        match self {
            Feature::EditorTabs => 0,
            _ => 2,
        }
    }

    pub fn all() -> &'static [Feature] {
        &[
            Feature::Ask,
            Feature::Agent,
            Feature::Sandbox,
            Feature::Git,
            Feature::EditorTabs,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum UsageResult {
    Allowed,
    Grace,
    Denied { used: u32, limit: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyData {
    date: String,
    counts: HashMap<String, u32>,
    grace_used: HashMap<String, u32>,
}

impl Default for DailyData {
    fn default() -> Self {
        Self {
            date: today_str(),
            counts: HashMap::new(),
            grace_used: HashMap::new(),
        }
    }
}

pub struct UsageTracker {
    data: DailyData,
    pro: bool,
}

impl UsageTracker {
    pub fn new(pro: bool) -> Self {
        let mut tracker = Self {
            data: DailyData::default(),
            pro,
        };
        tracker.load();
        tracker
    }

    pub fn set_pro(&mut self, pro: bool) {
        self.pro = pro;
    }

    pub fn is_pro(&self) -> bool {
        self.pro
    }

    pub fn can_use(&mut self, feature: Feature) -> UsageResult {
        self.ensure_today();

        if self.pro {
            return UsageResult::Allowed;
        }

        let limit = feature.free_limit();
        let used = self.count(feature);
        let grace = feature.grace_allowance();
        let grace_used = self.grace_count(feature);

        if used < limit {
            UsageResult::Allowed
        } else if grace > 0 && grace_used < grace {
            UsageResult::Grace
        } else {
            UsageResult::Denied { used, limit }
        }
    }

    pub fn record_use(&mut self, feature: Feature) {
        self.ensure_today();

        if self.pro {
            return;
        }

        let key = feature_key(feature);
        let limit = feature.free_limit();
        let used = self.count(feature);

        if used < limit {
            *self.data.counts.entry(key).or_insert(0) += 1;
        } else {
            *self.data.grace_used.entry(key).or_insert(0) += 1;
        }

        self.save();
    }

    pub fn count(&self, feature: Feature) -> u32 {
        self.data
            .counts
            .get(&feature_key(feature))
            .copied()
            .unwrap_or(0)
    }

    pub fn grace_count(&self, feature: Feature) -> u32 {
        self.data
            .grace_used
            .get(&feature_key(feature))
            .copied()
            .unwrap_or(0)
    }

    pub fn time_until_reset(&self) -> Duration {
        time_until_midnight()
    }

    fn ensure_today(&mut self) {
        let today = today_str();
        if self.data.date != today {
            self.data = DailyData {
                date: today,
                counts: HashMap::new(),
                grace_used: HashMap::new(),
            };
            self.save();
        }
    }

    fn load(&mut self) {
        let path = usage_path();
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(data) = serde_json::from_str::<DailyData>(&content)
            && data.date == today_str()
        {
            self.data = data;
        }
    }

    fn save(&self) {
        let path = usage_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&self.data) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn usage_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("awebo").join("usage.json")
}

fn feature_key(feature: Feature) -> String {
    match feature {
        Feature::Ask => "ask".into(),
        Feature::Agent => "agent".into(),
        Feature::Sandbox => "sandbox".into(),
        Feature::Git => "git".into(),
        Feature::EditorTabs => "editor_tabs".into(),
    }
}

fn today_str() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let utc_offset = local_utc_offset_secs();
    let local_secs = secs + utc_offset;
    let days = local_secs / 86400;
    let y = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y.0, y.1, y.2)
}

fn time_until_midnight() -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let utc_offset = local_utc_offset_secs();
    let local_secs = secs + utc_offset;
    let seconds_into_day = (local_secs % 86400) as u64;
    let remaining = 86400 - seconds_into_day;
    Duration::from_secs(remaining)
}

fn local_utc_offset_secs() -> i64 {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn localtime_r(timep: *const i64, result: *mut libc::tm) -> *mut libc::tm;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut tm = unsafe { std::mem::zeroed::<libc::tm>() };
        unsafe { localtime_r(&now, &mut tm) };
        tm.tm_gmtoff
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

pub fn format_duration_short(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {:02}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_labels() {
        assert_eq!(Feature::Ask.label(), "/ask");
        assert_eq!(Feature::Agent.label(), "/agent");
        assert_eq!(Feature::Sandbox.label(), "sandbox");
        assert_eq!(Feature::Git.label(), "git ops");
        assert_eq!(Feature::EditorTabs.label(), "editor tabs");
    }

    #[test]
    fn feature_limits() {
        assert_eq!(Feature::Ask.free_limit(), 20);
        assert_eq!(Feature::Agent.free_limit(), 5);
        assert_eq!(Feature::Sandbox.free_limit(), 3);
        assert_eq!(Feature::Git.free_limit(), 10);
        assert_eq!(Feature::EditorTabs.free_limit(), 2);
    }

    #[test]
    fn feature_keys_unique() {
        let keys: Vec<_> = Feature::all().iter().map(|f| feature_key(*f)).collect();
        for (i, k) in keys.iter().enumerate() {
            for (j, other) in keys.iter().enumerate() {
                if i != j {
                    assert_ne!(k, other);
                }
            }
        }
    }

    #[test]
    fn today_str_is_valid_date() {
        let s = today_str();
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }

    #[test]
    fn time_until_midnight_is_positive() {
        let d = time_until_midnight();
        assert!(d.as_secs() > 0);
        assert!(d.as_secs() <= 86400);
    }

    #[test]
    fn format_duration_hours() {
        let d = Duration::from_secs(4 * 3600 + 12 * 60);
        assert_eq!(format_duration_short(d), "4h 12m");
    }

    #[test]
    fn format_duration_minutes() {
        let d = Duration::from_secs(45 * 60);
        assert_eq!(format_duration_short(d), "45m");
    }

    #[test]
    fn usage_tracker_pro_always_allowed() {
        let mut tracker = UsageTracker {
            data: DailyData::default(),
            pro: true,
        };
        for _ in 0..100 {
            assert!(matches!(
                tracker.can_use(Feature::Ask),
                UsageResult::Allowed
            ));
        }
    }

    #[test]
    fn daily_data_resets_on_new_day() {
        let mut data = DailyData {
            date: "2020-01-01".into(),
            counts: HashMap::from([("ask".into(), 15)]),
            grace_used: HashMap::new(),
        };
        let today = today_str();
        if data.date != today {
            data = DailyData {
                date: today,
                ..Default::default()
            };
        }
        assert!(data.counts.is_empty());
    }
}
