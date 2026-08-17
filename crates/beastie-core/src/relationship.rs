use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ACTIVE_DAY_MS, BeliefKind, FoodId, Memory, MemoryId, MemoryKind, RelationshipBeat,
    RelationshipBeatPhase, RelationshipEvidence, RelationshipExpressionKind, RelationshipMotif,
    RelationshipMotifKey, RelationshipTrigger, RelationshipTriggerKind, SemanticDestination, ToyId,
    WorldState,
};

pub const MAX_MOTIF_EVIDENCE: usize = 8;
pub const MAX_RECENT_EXPRESSIONS: usize = 8;
pub const MAX_RELATIONSHIP_BEATS_PER_DAY: u8 = 4;
pub const RELATIONSHIP_GLOBAL_COOLDOWN_MS: u64 = 20_000;
pub const RELATIONSHIP_MOTIF_COOLDOWN_MS: u64 = 90_000;

#[derive(Default)]
struct MemoryEvidence {
    ids: Vec<MemoryId>,
    days: BTreeSet<u64>,
    last_day: u64,
}

impl MemoryEvidence {
    fn push(&mut self, memory: &Memory) {
        let day = memory.happened_at_ms / ACTIVE_DAY_MS + 1;
        self.ids.push(memory.id);
        self.days.insert(day);
        self.last_day = self.last_day.max(day);
    }

    fn evidence(&self) -> Vec<RelationshipEvidence> {
        self.ids
            .iter()
            .rev()
            .take(MAX_MOTIF_EVIDENCE)
            .map(|id| RelationshipEvidence::Memory { id: *id })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn motif(&self, key: RelationshipMotifKey) -> RelationshipMotif {
        RelationshipMotif {
            key,
            strength: strength(self.days.len()),
            evidence: self.evidence(),
            last_supported_day: self.last_day,
            eligible_triggers: triggers_for(key),
        }
    }
}

fn strength(days: usize) -> u8 {
    match days {
        0 => 0,
        1 => 1,
        2..=3 => 2,
        _ => 3,
    }
}

fn with_supporting_belief(
    mut motif: RelationshipMotif,
    state: &WorldState,
    kind: BeliefKind,
) -> RelationshipMotif {
    let Some(belief) = state.creature.beliefs.iter().find(|belief| {
        belief.kind == kind
            && motif.evidence.iter().any(|evidence| {
                matches!(
                    evidence,
                    RelationshipEvidence::Memory { id }
                        if belief.supporting_memories.contains(id)
                )
            })
    }) else {
        return motif;
    };
    let keep = MAX_MOTIF_EVIDENCE.saturating_sub(1);
    if motif.evidence.len() > keep {
        let remove = motif.evidence.len() - keep;
        motif.evidence.drain(..remove);
    }
    motif.evidence.push(RelationshipEvidence::Belief {
        id: belief.id,
        kind,
    });
    motif
}

fn triggers_for(key: RelationshipMotifKey) -> BTreeSet<RelationshipTriggerKind> {
    use RelationshipTriggerKind as Trigger;
    match key {
        RelationshipMotifKey::SharedToy(_) => [
            Trigger::PlayerReturn,
            Trigger::FamiliarObject,
            Trigger::RelevantUtterance,
            Trigger::ActionCompleted,
        ]
        .into_iter()
        .collect(),
        RelationshipMotifKey::ComfortRitual => [
            Trigger::NeedState,
            Trigger::RelevantUtterance,
            Trigger::ActionCompleted,
        ]
        .into_iter()
        .collect(),
        RelationshipMotifKey::TrustedFood(_) | RelationshipMotifKey::FoodGrudge(_) => [
            Trigger::FamiliarObject,
            Trigger::RelevantUtterance,
            Trigger::ActionCompleted,
        ]
        .into_iter()
        .collect(),
        RelationshipMotifKey::PlayerReturns => [Trigger::PlayerReturn, Trigger::RelevantUtterance]
            .into_iter()
            .collect(),
        RelationshipMotifKey::FamiliarPlace(_) => [
            Trigger::PlayerReturn,
            Trigger::RoutineWindow,
            Trigger::RelevantUtterance,
        ]
        .into_iter()
        .collect(),
    }
}

/// Derive motifs from canonical memories, beliefs, preferences, and routine evidence.
#[must_use]
pub fn derive_relationship_motifs(state: &WorldState) -> Vec<RelationshipMotif> {
    let mut toys = BTreeMap::<ToyId, MemoryEvidence>::new();
    let mut fed = BTreeMap::<FoodId, MemoryEvidence>::new();
    let mut grudges = BTreeMap::<FoodId, MemoryEvidence>::new();
    let mut comfort = MemoryEvidence::default();
    let mut returns = MemoryEvidence::default();

    for memory in &state.creature.memories {
        match memory.kind {
            MemoryKind::PlayedWith { toy } => toys.entry(toy).or_default().push(memory),
            MemoryKind::WasFed { food } => fed.entry(food).or_default().push(memory),
            MemoryKind::DislikedFood { food } | MemoryKind::RejectedFood { food } => {
                grudges.entry(food).or_default().push(memory)
            }
            MemoryKind::WasComforted => comfort.push(memory),
            MemoryKind::PlayerReturnedAfterAbsence => returns.push(memory),
            MemoryKind::DislikedToy { .. } | MemoryKind::PlayerReacted { .. } => {}
        }
    }

    let mut motifs = Vec::new();
    motifs.extend(
        toys.into_iter()
            .filter(|(_, evidence)| !evidence.days.is_empty())
            .map(|(toy, evidence)| evidence.motif(RelationshipMotifKey::SharedToy(toy))),
    );
    if !comfort.days.is_empty() {
        motifs.push(comfort.motif(RelationshipMotifKey::ComfortRitual));
    }
    motifs.extend(fed.into_iter().filter_map(|(food, evidence)| {
        let preference = state
            .creature
            .preferences
            .get(&food)
            .copied()
            .unwrap_or_default();
        (evidence.days.len() >= 2 && preference >= 0.15)
            .then(|| evidence.motif(RelationshipMotifKey::TrustedFood(food)))
    }));
    motifs.extend(grudges.into_iter().filter_map(|(food, evidence)| {
        let preference = state
            .creature
            .preferences
            .get(&food)
            .copied()
            .unwrap_or_default();
        let belief_supported = state.creature.beliefs.iter().any(|belief| {
            belief.kind == BeliefKind::RedFoodIsATrick
                && evidence
                    .ids
                    .iter()
                    .any(|id| belief.supporting_memories.contains(id))
        });
        (preference <= -0.15 && belief_supported).then(|| {
            with_supporting_belief(
                evidence.motif(RelationshipMotifKey::FoodGrudge(food)),
                state,
                BeliefKind::RedFoodIsATrick,
            )
        })
    }));
    let return_belief_supported = state.creature.beliefs.iter().any(|belief| {
        belief.kind == BeliefKind::PlayerReturnsAfterSleep
            && returns
                .ids
                .iter()
                .any(|id| belief.supporting_memories.contains(id))
    });
    if !returns.ids.is_empty() && return_belief_supported {
        let motif = with_supporting_belief(
            returns.motif(RelationshipMotifKey::PlayerReturns),
            state,
            BeliefKind::PlayerReturnsAfterSleep,
        );
        motifs.push(motif);
    }

    for routine in &state.creature.routines {
        let Some(evidence) = state
            .creature
            .idle_life
            .visit_evidence
            .iter()
            .find(|visit| {
                visit.hour_start == routine.hour_start && visit.destination == routine.destination
            })
        else {
            continue;
        };
        motifs.push(RelationshipMotif {
            key: RelationshipMotifKey::FamiliarPlace(routine.destination),
            strength: routine.strength.min(3),
            evidence: vec![RelationshipEvidence::Visit {
                hour_start: evidence.hour_start,
                destination: evidence.destination,
                last_active_day: evidence.last_active_day,
            }],
            last_supported_day: evidence.last_active_day,
            eligible_triggers: triggers_for(RelationshipMotifKey::FamiliarPlace(
                routine.destination,
            )),
        });
    }

    motifs.sort_by_key(|motif| motif.key);
    motifs.dedup_by(|left, right| {
        if left.key != right.key {
            return false;
        }
        if right.strength > left.strength {
            *left = right.clone();
        }
        true
    });
    motifs
}

fn trigger_relevance(key: RelationshipMotifKey, trigger: RelationshipTrigger) -> u32 {
    match (key, trigger) {
        (RelationshipMotifKey::SharedToy(a), RelationshipTrigger::FamiliarToy { toy: b })
        | (
            RelationshipMotifKey::SharedToy(a),
            RelationshipTrigger::ActionCompleted {
                destination: SemanticDestination::Toy(b),
            },
        ) if a == b => 250,
        (RelationshipMotifKey::TrustedFood(a), RelationshipTrigger::FamiliarFood { food: b })
        | (RelationshipMotifKey::FoodGrudge(a), RelationshipTrigger::FamiliarFood { food: b })
            if a == b =>
        {
            250
        }
        (
            RelationshipMotifKey::FamiliarPlace(a),
            RelationshipTrigger::RoutineWindow { destination: b, .. },
        ) if a == b => 250,
        (RelationshipMotifKey::ComfortRitual, RelationshipTrigger::NeedState) => 240,
        (RelationshipMotifKey::PlayerReturns, RelationshipTrigger::PlayerReturn) => 250,
        (_, RelationshipTrigger::RelevantUtterance) => 80,
        (_, RelationshipTrigger::PlayerReturn) => 35,
        (_, RelationshipTrigger::ActionCompleted { .. }) => 25,
        _ => 0,
    }
}

fn expression_for(
    motif: &RelationshipMotif,
    trigger: RelationshipTrigger,
) -> RelationshipExpressionKind {
    match motif.key {
        RelationshipMotifKey::ComfortRitual if motif.strength >= 3 => {
            RelationshipExpressionKind::Ritual
        }
        RelationshipMotifKey::ComfortRitual
            if motif.strength >= 2 && matches!(trigger, RelationshipTrigger::NeedState) =>
        {
            RelationshipExpressionKind::Seek
        }
        RelationshipMotifKey::ComfortRitual if motif.strength >= 2 => {
            RelationshipExpressionKind::Anticipate
        }
        RelationshipMotifKey::ComfortRitual => RelationshipExpressionKind::Notice,
        RelationshipMotifKey::PlayerReturns if motif.strength >= 3 => {
            RelationshipExpressionKind::Welcome
        }
        RelationshipMotifKey::PlayerReturns if motif.strength >= 2 => {
            RelationshipExpressionKind::Anticipate
        }
        RelationshipMotifKey::PlayerReturns => RelationshipExpressionKind::Notice,
        RelationshipMotifKey::FamiliarPlace(_) => RelationshipExpressionKind::Recognize,
        _ if motif.strength >= 3 => RelationshipExpressionKind::Ritual,
        _ if motif.strength >= 2 => RelationshipExpressionKind::Anticipate,
        _ => RelationshipExpressionKind::Notice,
    }
}

fn target_for(key: RelationshipMotifKey) -> Option<SemanticDestination> {
    match key {
        RelationshipMotifKey::SharedToy(toy) => Some(SemanticDestination::Toy(toy)),
        RelationshipMotifKey::ComfortRitual | RelationshipMotifKey::PlayerReturns => {
            Some(SemanticDestination::Player)
        }
        RelationshipMotifKey::TrustedFood(_) | RelationshipMotifKey::FoodGrudge(_) => {
            Some(SemanticDestination::Bottom)
        }
        RelationshipMotifKey::FamiliarPlace(destination) => Some(destination),
    }
}

pub(crate) fn relationship_beat_is_grounded(state: &WorldState, beat: &RelationshipBeat) -> bool {
    let Some(motif) = derive_relationship_motifs(state)
        .into_iter()
        .find(|motif| motif.key == beat.motif)
    else {
        return false;
    };
    let relevant = trigger_relevance(motif.key, beat.trigger) > 0
        || matches!(beat.trigger, RelationshipTrigger::RelevantUtterance);
    relevant
        && motif.eligible_triggers.contains(&beat.trigger.kind())
        && beat.expression_kind == expression_for(&motif, beat.trigger)
        && beat.evidence == motif.evidence
        && beat.target == target_for(motif.key)
}

fn scaled_trait(value: f32, weight: u32) -> u32 {
    (value.clamp(0.0, 1.0) * weight as f32).round() as u32
}

fn temperament_score(state: &WorldState, key: RelationshipMotifKey) -> u32 {
    let traits = state.creature.traits;
    match key {
        RelationshipMotifKey::SharedToy(_) => {
            scaled_trait(traits.boldness, 14)
                + scaled_trait(traits.sociability, 8)
                + scaled_trait(1.0 - traits.repetitiveness, 8)
        }
        RelationshipMotifKey::ComfortRitual => {
            scaled_trait(traits.sociability, 16)
                + scaled_trait(traits.boldness, 5)
                + scaled_trait(1.0 - traits.stubbornness, 6)
        }
        RelationshipMotifKey::TrustedFood(_) => {
            scaled_trait(1.0 - traits.fussiness, 12)
                + scaled_trait(traits.boldness, 7)
                + scaled_trait(traits.repetitiveness, 4)
        }
        RelationshipMotifKey::FoodGrudge(_) => {
            scaled_trait(traits.fussiness, 14)
                + scaled_trait(traits.stubbornness, 14)
                + scaled_trait(traits.literalness, 6)
        }
        RelationshipMotifKey::PlayerReturns => {
            scaled_trait(traits.sociability, 15) + scaled_trait(traits.boldness, 10)
        }
        RelationshipMotifKey::FamiliarPlace(_) => {
            scaled_trait(traits.repetitiveness, 14) + scaled_trait(traits.literalness, 8)
        }
    }
}

fn relationship_score(state: &WorldState) -> u32 {
    let relationship = state.creature.relationship;
    scaled_trait(relationship.bond, 30)
        + scaled_trait(relationship.trust, 25)
        + scaled_trait(relationship.respect, 15)
        + scaled_trait(1.0 - relationship.resentment, 10)
}

fn evidence_recency_score(state: &WorldState, motif: &RelationshipMotif) -> u32 {
    let age_days = state
        .active_day()
        .saturating_sub(motif.last_supported_day)
        .min(20);
    20u64.saturating_sub(age_days) as u32
}

fn time_since_last_expression_score(state: &WorldState) -> u32 {
    state
        .creature
        .relationship_expression
        .last_expressed_at_ms
        .map(|time| state.elapsed_ms.saturating_sub(time) / 1_000)
        .unwrap_or(90)
        .min(90) as u32
}

/// Deterministically select one eligible motif. This is read-only; starting a beat mutates the
/// ledger separately so callers can test scoring without changing world state.
#[must_use]
pub fn select_relationship_beat(
    state: &WorldState,
    trigger: RelationshipTrigger,
) -> Option<RelationshipBeat> {
    let expression = &state.creature.relationship_expression;
    if expression.active.is_some() {
        return None;
    }
    let day = state.active_day();
    let day_count = if expression.count_active_day_index == day {
        expression.count_active_day
    } else {
        0
    };
    if day_count >= crate::relationship::MAX_RELATIONSHIP_BEATS_PER_DAY
        || expression.last_expressed_at_ms.is_some_and(|time| {
            state.elapsed_ms.saturating_sub(time) < RELATIONSHIP_GLOBAL_COOLDOWN_MS
        })
    {
        return None;
    }
    let motifs = derive_relationship_motifs(state);
    let mut candidates = motifs
        .into_iter()
        .filter(|motif| motif.eligible_triggers.contains(&trigger.kind()))
        .filter(|motif| {
            !expression.recent.iter().any(|recent| {
                recent.key == motif.key
                    && state.elapsed_ms.saturating_sub(recent.expressed_at_ms)
                        < RELATIONSHIP_MOTIF_COOLDOWN_MS
            })
        })
        .filter_map(|motif| {
            let relevance = trigger_relevance(motif.key, trigger);
            if relevance == 0 && !matches!(trigger, RelationshipTrigger::RelevantUtterance) {
                return None;
            }
            let score = u32::from(motif.strength) * 100
                + relevance
                + evidence_recency_score(state, &motif)
                + temperament_score(state, motif.key)
                + relationship_score(state)
                + time_since_last_expression_score(state);
            Some((score, motif))
        })
        .collect::<Vec<_>>();

    // A long offline gap expires elapsed-time cooldowns. Keep consecutive meaningful
    // triggers from becoming a one-motif loop when another grounded callback is available,
    // while still allowing the only valid motif to speak.
    if candidates.len() > 1
        && let Some(last_key) = expression.recent.last().map(|recent| recent.key)
    {
        let has_alternative = candidates.iter().any(|(_, motif)| motif.key != last_key);
        if has_alternative {
            candidates.retain(|(_, motif)| motif.key != last_key);
        }
    }

    candidates
        .into_iter()
        .max_by(|(left_score, left), (right_score, right)| {
            left_score
                .cmp(right_score)
                .then_with(|| right.key.cmp(&left.key))
        })
        .map(|(_, motif)| RelationshipBeat {
            motif: motif.key,
            trigger,
            expression_kind: expression_for(&motif, trigger),
            evidence: motif.evidence,
            target: target_for(motif.key),
            phase: RelationshipBeatPhase::Notice,
            started_at_ms: state.elapsed_ms,
            phase_started_at_ms: state.elapsed_ms,
        })
}

#[must_use]
pub const fn phase_duration_ms(phase: RelationshipBeatPhase) -> u64 {
    match phase {
        RelationshipBeatPhase::Notice => 1_000,
        RelationshipBeatPhase::Anticipate => 2_000,
        RelationshipBeatPhase::Act => 2_000,
        RelationshipBeatPhase::Recover => 2_000,
    }
}
