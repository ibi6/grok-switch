//! Provider pool selection for failover / weighted routing.

use crate::core::types::{PoolStrategy, Provider};
use chrono::Local;

/// Providers eligible for automatic selection right now.
pub fn eligible_providers(all: &[Provider]) -> Vec<Provider> {
    let now = Local::now().timestamp();
    let mut out: Vec<Provider> = all
        .iter()
        .filter(|p| p.pool_enabled)
        .filter(|p| p.cooldown_until.map(|t| t <= now).unwrap_or(true))
        .filter(|p| !p.api_key.trim().is_empty() && !p.base_url.trim().is_empty())
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// Pick ordered candidates according to strategy (first = preferred).
pub fn order_candidates(all: &[Provider], strategy: PoolStrategy) -> Vec<Provider> {
    let mut list = eligible_providers(all);
    match strategy {
        PoolStrategy::Priority => {
            // already sorted by priority
        }
        PoolStrategy::Weighted => {
            // Stable weighted shuffle: expand by weight buckets then unique.
            // Simple deterministic approach: sort by (priority desc, weight desc).
            list.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| b.weight.cmp(&a.weight))
                    .then_with(|| a.name.cmp(&b.name))
            });
        }
        PoolStrategy::RoundRobin => {
            // Prefer least-recently-updated as a stand-in for "last used".
            list.sort_by(|a, b| a.updated_at.cmp(&b.updated_at).then_with(|| a.name.cmp(&b.name)));
        }
    }
    list
}

/// Mark a provider into cooldown for `secs` seconds (returns updated clone).
pub fn with_cooldown(provider: &Provider, secs: i64) -> Provider {
    let mut p = provider.clone();
    p.cooldown_until = Some(Local::now().timestamp() + secs.max(1));
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ApiBackend, ProviderSource};

    fn p(id: &str, priority: i32, weight: u32, enabled: bool, cooldown: Option<i64>) -> Provider {
        Provider {
            id: id.into(),
            name: id.into(),
            base_url: "https://x/v1".into(),
            api_key: "sk-test-key-abcdefghijklmnop".into(),
            api_backend: ApiBackend::ChatCompletions,
            default_model_entry_id: "m".into(),
            models: vec![],
            extra_headers: None,
            context_window: 1000,
            website_url: None,
            notes: None,
            source: ProviderSource::Manual,
            created_at: 1,
            updated_at: 1,
            priority,
            weight,
            pool_enabled: enabled,
            cooldown_until: cooldown,
        }
    }

    #[test]
    fn filters_disabled_and_cooldown() {
        let now = Local::now().timestamp();
        let all = vec![
            p("a", 10, 100, true, None),
            p("b", 5, 100, false, None),
            p("c", 20, 100, true, Some(now + 3600)),
            p("d", 1, 100, true, Some(now - 10)),
        ];
        let el = eligible_providers(&all);
        let ids: Vec<_> = el.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "d"]); // c cooled, b disabled; priority a > d
    }

    #[test]
    fn priority_order() {
        let all = vec![p("low", 1, 10, true, None), p("high", 50, 1, true, None)];
        let ordered = order_candidates(&all, PoolStrategy::Priority);
        assert_eq!(ordered[0].id, "high");
    }
}
