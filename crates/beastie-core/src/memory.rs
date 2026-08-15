use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{ACTIVE_DAY_MS, Concept, FoodId, Memory, MemoryKind, WorldState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MemoryCue {
    Concept(Concept),
    Food(FoodId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryQuery {
    pub cues: BTreeSet<MemoryCue>,
    pub limit: usize,
}

#[must_use]
pub fn select_candidate_memories(state: &WorldState, query: &MemoryQuery) -> Vec<Memory> {
    let now = state.elapsed_ms;
    let relevant_concepts = query
        .cues
        .iter()
        .filter_map(|cue| match cue {
            MemoryCue::Concept(concept) => Some(*concept),
            MemoryCue::Food(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let relevant_food = query
        .cues
        .iter()
        .filter_map(|cue| match cue {
            MemoryCue::Food(food) => Some(*food),
            MemoryCue::Concept(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let mut ranked = state
        .creature
        .memories
        .iter()
        .map(|memory| {
            let age_days = now.saturating_sub(memory.happened_at_ms) as f32 / ACTIVE_DAY_MS as f32;
            let recency = 1.0 / (1.0 + age_days);
            let relevance = memory.concepts.intersection(&relevant_concepts).count() as f32;
            let subject_match = f32::from(
                memory_food(&memory.kind).is_some_and(|food| relevant_food.contains(&food)),
            );
            let score = memory.salience * 2.0
                + memory.valence.abs() * 0.5
                + recency
                + relevance
                + subject_match * 3.0;
            (memory, score)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_memory, left_score), (right_memory, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| right_memory.id.cmp(&left_memory.id))
    });
    ranked
        .into_iter()
        .take(query.limit.min(8))
        .map(|(memory, _)| memory.clone())
        .collect()
}

fn memory_food(kind: &MemoryKind) -> Option<FoodId> {
    match kind {
        MemoryKind::WasFed { food }
        | MemoryKind::DislikedFood { food }
        | MemoryKind::RejectedFood { food } => Some(*food),
        MemoryKind::Played
        | MemoryKind::WasComforted
        | MemoryKind::PlayerReturnedAfterAbsence
        | MemoryKind::PlayerReacted { .. } => None,
    }
}
