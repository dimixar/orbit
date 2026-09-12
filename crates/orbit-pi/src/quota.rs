//! Account-level provider quota, balance, and spend for Settings → Providers.
//!
//! Orbit performs no provider HTTP and never sees a credential. The pi RPC
//! server resolves each connected provider's auth, queries the provider's own
//! usage endpoint, and returns normalized, non-secret reports (see
//! `crates/orbit-rpc/docs/quota-rpc.md`). This module is a small reducer over
//! the `quota.list` response:
//!
//! - It records whether the running pi build exposes `quota.*` at all.
//! - It caches one [`QuotaReport`] per provider for the UI to render.
//! - It performs no I/O and touches no network; the GPUI layer owns the RPC.
//!
//! The report carries only display facts — percentages, counts, reset times,
//! and monetary balances — so there is no secret boundary to police here.

use std::collections::HashMap;

use orbit_rpc::{parse_quota_reports, QuotaReport};

/// Whether pi exposes the quota RPC namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaSupport {
    /// Not probed yet (or the process restarted; probe again).
    Unknown,
    /// `quota.list` answered successfully.
    Supported,
    /// pi rejected `quota.list`; the Providers page hides quota meters.
    Unsupported,
}

/// The non-secret quota cache, keyed by provider id.
pub struct QuotaManager {
    support: QuotaSupport,
    reports: HashMap<String, QuotaReport>,
}

impl Default for QuotaManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaManager {
    pub fn new() -> Self {
        Self {
            support: QuotaSupport::Unknown,
            reports: HashMap::new(),
        }
    }

    pub fn support(&self) -> QuotaSupport {
        self.support
    }

    /// The cached report for a provider, if one has been fetched.
    pub fn report(&self, id: &str) -> Option<&QuotaReport> {
        self.reports.get(id)
    }

    /// Every cached report, ordered by provider id for a stable UI. Reports
    /// with nothing to render (no windows, balances, note, or error) are
    /// omitted so the top-bar summary never counts an empty provider.
    pub fn reports(&self) -> Vec<&QuotaReport> {
        let mut reports: Vec<&QuotaReport> = self
            .reports
            .values()
            .filter(|report| report.has_data() || report.error.is_some() || report.note.is_some())
            .collect();
        reports.sort_by(|a, b| a.provider.cmp(&b.provider));
        reports
    }

    /// The single most-constraining window across every provider: the highest
    /// fraction used, preferring a window without an error. `None` when no
    /// provider reports a percentage or a used/limit pair. Used for the
    /// top-bar headroom accent, never to fabricate a value.
    pub fn peak_window(&self) -> Option<(&QuotaReport, &orbit_rpc::QuotaWindow)> {
        let mut peak: Option<(&QuotaReport, &orbit_rpc::QuotaWindow)> = None;
        for report in self.reports() {
            if report.error.is_some() {
                continue;
            }
            for window in &report.windows {
                let Some(fraction) = window.fraction() else {
                    continue;
                };
                let better = peak
                    .as_ref()
                    .map(|(_, best)| fraction > best.fraction().unwrap_or(0.0))
                    .unwrap_or(true);
                if better {
                    peak = Some((report, window));
                }
            }
        }
        peak
    }

    /// Apply a `quota.list` response. A namespace-level failure marks quota
    /// unsupported; a successful response merges each report by provider id,
    /// so a single-provider refresh does not drop the others.
    pub fn on_response(
        &mut self,
        success: bool,
        data: Option<&serde_json::Value>,
        error: Option<&str>,
    ) {
        if !success {
            if error.is_some_and(is_unsupported_error) {
                self.support = QuotaSupport::Unsupported;
            }
            return;
        }
        self.support = QuotaSupport::Supported;
        let Some(data) = data else {
            return;
        };
        for report in parse_quota_reports(data) {
            self.reports.insert(report.provider.clone(), report);
        }
    }

    /// The pi process went away: forget the capability probe so the next
    /// process is re-checked, but keep the last reports so cards do not blink
    /// empty during a restart.
    pub fn on_disconnect(&mut self) {
        self.support = QuotaSupport::Unknown;
    }
}

fn is_unsupported_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("unknown command") || lower.contains("unsupported")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> serde_json::Value {
        json!({
            "providers": [
                {"provider":"anthropic","kind":"subscription","plan":"Max",
                 "windows":[{"id":"five_hour","label":"5-hour","usedPercent":6.0,"resetsAt":1738300000000i64}]},
                {"provider":"deepseek","kind":"balance","windows":[],
                 "balances":[{"label":"Available","amount":110.0,"currency":"CNY"}]}
            ]
        })
    }

    #[test]
    fn parses_and_caches_reports() {
        let mut quota = QuotaManager::new();
        assert_eq!(quota.support(), QuotaSupport::Unknown);
        quota.on_response(true, Some(&payload()), None);
        assert_eq!(quota.support(), QuotaSupport::Supported);
        let anthropic = quota.report("anthropic").expect("anthropic report");
        assert_eq!(anthropic.plan.as_deref(), Some("Max"));
        assert_eq!(anthropic.windows.len(), 1);
        assert!(quota.report("deepseek").unwrap().has_data());
    }

    #[test]
    fn merges_single_provider_refresh() {
        let mut quota = QuotaManager::new();
        quota.on_response(true, Some(&payload()), None);
        quota.on_response(
            true,
            Some(
                &json!({"providers":[{"provider":"anthropic","kind":"subscription",
                "windows":[{"id":"weekly","label":"Weekly","usedPercent":50.0}]}]}),
            ),
            None,
        );
        // The refreshed provider is replaced; the untouched one survives.
        assert_eq!(quota.report("anthropic").unwrap().windows[0].id, "weekly");
        assert!(quota.report("deepseek").is_some());
    }

    #[test]
    fn unsupported_pi_marks_fallback_and_ignores_errors() {
        let mut quota = QuotaManager::new();
        quota.on_response(false, None, Some("Unknown command: quota.list"));
        assert_eq!(quota.support(), QuotaSupport::Unsupported);
        // An unrelated command error must not flip support.
        let mut quota = QuotaManager::new();
        quota.on_response(false, None, Some("network hiccup"));
        assert_eq!(quota.support(), QuotaSupport::Unknown);
    }

    #[test]
    fn disconnect_keeps_reports_but_reprobes() {
        let mut quota = QuotaManager::new();
        quota.on_response(true, Some(&payload()), None);
        quota.on_disconnect();
        assert_eq!(quota.support(), QuotaSupport::Unknown);
        assert!(quota.report("anthropic").is_some());
    }

    #[test]
    fn reports_are_sorted_and_skip_empty_entries() {
        let mut quota = QuotaManager::new();
        quota.on_response(
            true,
            Some(&json!({"providers":[
                {"provider":"zai","kind":"subscription",
                 "windows":[{"id":"5h","label":"5-hour","usedPercent":10.0,"resetsAt":1}]},
                {"provider":"anthropic","kind":"subscription",
                 "windows":[{"id":"7d","label":"Weekly","usedPercent":20.0,"resetsAt":2}]},
                {"provider":"groq","kind":"unsupported","windows":[],"balances":[]}
            ]})),
            None,
        );
        let providers: Vec<&str> = quota
            .reports()
            .iter()
            .map(|r| r.provider.as_str())
            .collect();
        // Sorted by id; the empty groq report is omitted.
        assert_eq!(providers, vec!["anthropic", "zai"]);
    }

    #[test]
    fn peak_window_picks_the_highest_fraction() {
        let mut quota = QuotaManager::new();
        quota.on_response(
            true,
            Some(&json!({"providers":[
                {"provider":"anthropic","kind":"subscription",
                 "windows":[{"id":"5h","label":"5-hour","usedPercent":6.0,"resetsAt":1}]},
                {"provider":"zai","kind":"subscription",
                 "windows":[{"id":"weekly","label":"Weekly","usedPercent":82.0,"resetsAt":2}]}
            ]})),
            None,
        );
        let (report, window) = quota.peak_window().expect("a peak");
        assert_eq!(report.provider, "zai");
        assert_eq!(window.id, "weekly");
        assert!((window.fraction().unwrap() - 0.82).abs() < 1e-6);
    }

    #[test]
    fn peak_window_ignores_errored_reports() {
        let mut quota = QuotaManager::new();
        quota.on_response(
            true,
            Some(&json!({"providers":[
                {"provider":"anthropic","kind":"subscription","error":"429",
                 "windows":[{"id":"5h","label":"5-hour","usedPercent":99.0,"resetsAt":1}]},
                {"provider":"zai","kind":"subscription",
                 "windows":[{"id":"weekly","label":"Weekly","usedPercent":40.0,"resetsAt":2}]}
            ]})),
            None,
        );
        let (report, _) = quota.peak_window().expect("a peak");
        assert_eq!(report.provider, "zai");
    }

    #[test]
    fn peak_window_is_none_for_balance_only() {
        let mut quota = QuotaManager::new();
        quota.on_response(true, Some(&payload()), None);
        // Only deepseek has a balance and anthropic has a percentage, so the
        // peak is anthropic's window, not None.
        assert_eq!(quota.peak_window().unwrap().0.provider, "anthropic");

        let mut balances = QuotaManager::new();
        balances.on_response(
            true,
            Some(
                &json!({"providers":[{"provider":"deepseek","kind":"balance",
                "balances":[{"label":"Available","amount":110.0,"currency":"CNY"}]}]}),
            ),
            None,
        );
        assert!(balances.peak_window().is_none());
        assert_eq!(balances.reports().len(), 1);
    }
}
