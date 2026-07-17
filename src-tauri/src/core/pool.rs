//! Provider pool selection for failover / weighted / round-robin routing.

use crate::core::db;
use crate::core::paths::Paths;
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
///
/// `paths` is required for round-robin counter persistence; pass `None` in pure tests.
pub fn order_candidates(
    all: &[Provider],
    strategy: PoolStrategy,
    paths: Option<&Paths>,
) -> Vec<Provider> {
    let mut list = eligible_providers(all);
    if list.is_empty() {
        return list;
    }
    match strategy {
        PoolStrategy::Priority => {
            // already sorted by priority
        }
        PoolStrategy::Weighted => {
            // Weighted random pick for head, then remaining by priority.
            if let Some(idx) = weighted_pick_index(&list) {
                let chosen = list.remove(idx);
                list.insert(0, chosen);
            }
        }
        PoolStrategy::RoundRobin => {
            // Rotate by persistent counter so each request advances the head.
            let n = list.len() as u64;
            let counter = paths
                .and_then(|p| db::next_pool_counter(p, "provider_rr").ok())
                .unwrap_or(0);
            let start = (counter % n) as usize;
            if start > 0 {
                list.rotate_left(start);
            }
        }
    }
    list
}

fn weighted_pick_index(list: &[Provider]) -> Option<usize> {
    let total: u64 = list.iter().map(|p| p.weight.max(1) as u64).sum();
    if total == 0 {
        return None;
    }
    // Cheap deterministic-ish mix from time nanos + id hash — good enough for local pool.
    let tick = Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let mut x = tick
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(list.len() as u64);
    x ^= x >> 33;
    let mut r = x % total;
    for (i, p) in list.iter().enumerate() {
        let w = p.weight.max(1) as u64;
        if r < w {
            return Some(i);
        }
        r -= w;
    }
    Some(0)
}

/// Mark a provider into cooldown for `secs` seconds (returns updated clone).
pub fn with_cooldown(provider: &Provider, secs: i64) -> Provider {
    let mut p = provider.clone();
    p.cooldown_until = Some(Local::now().timestamp() + secs.max(1));
    p
}

/// Clear cooldown so the provider re-enters the pool immediately.
pub fn clear_cooldown(provider: &Provider) -> Provider {
    let mut p = provider.clone();
    p.cooldown_until = None;
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
        let ordered = order_candidates(&all, PoolStrategy::Priority, None);
        assert_eq!(ordered[0].id, "high");
    }

    #[test]
    fn weighted_always_returns_someone() {
        let all = vec![
            p("a", 0, 1, true, None),
            p("b", 0, 99, true, None),
        ];
        let ordered = order_candidates(&all, PoolStrategy::Weighted, None);
        assert_eq!(ordered.len(), 2);
        assert!(ordered[0].id == "a" || ordered[0].id == "b");
    }

    #[test]
    fn clear_cooldown_works() {
        let now = Local::now().timestamp();
        let cooled = p("x", 0, 1, true, Some(now + 100));
        let cleared = clear_cooldown(&cooled);
        assert!(cleared.cooldown_until.is_none());
    }
}
