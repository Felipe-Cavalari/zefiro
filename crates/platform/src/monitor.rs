//! Identificação persistente, separada da posição e da ordem temporária de enumeração.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorIdentity {
    pub id: Option<String>,
    pub name: String,
}

/// IDs repetidos não podem associar dois monitores ao mesmo layout próprio.
pub fn reject_ambiguous(identities: &mut [MonitorIdentity]) {
    let mut counts = std::collections::HashMap::new();
    for id in identities.iter().filter_map(|m| m.id.as_deref()) {
        *counts.entry(id.to_string()).or_insert(0) += 1;
    }
    for identity in identities {
        if identity.id.as_ref().is_some_and(|id| counts[id] > 1) { identity.id = None; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_names_keep_distinct_ids_but_duplicate_paths_are_rejected() {
        let monitor = |id: &str| MonitorIdentity { id: Some(id.into()), name: "Mesmo modelo".into() };
        let mut ids = [monitor("path-a"), monitor("path-b"), monitor("path-c"), monitor("path-c")];
        reject_ambiguous(&mut ids);
        assert_eq!(ids[0].id.as_deref(), Some("path-a"));
        assert_eq!(ids[1].id.as_deref(), Some("path-b"));
        assert!(ids[2].id.is_none() && ids[3].id.is_none());
    }
}
