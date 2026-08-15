//! Authoritative, deterministic creature simulation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemoryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoodId {
    Berry,
    Mushroom,
    Pellet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concept {
    SelfIdentity,
    You,
    Food,
    Good,
    Bad,
    Here,
    Sleep,
    Toy,
    Again,
    Yesterday,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub energy: f32,
    pub comfort: f32,
    pub curiosity: f32,
}

impl Needs {
    fn clamp(&mut self) {
        self.hunger = self.hunger.clamp(0.0, 1.0);
        self.energy = self.energy.clamp(0.0, 1.0);
        self.comfort = self.comfort.clamp(0.0, 1.0);
        self.curiosity = self.curiosity.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Traits {
    pub sociability: f32,
    pub boldness: f32,
    pub fussiness: f32,
    pub stubbornness: f32,
    pub literalness: f32,
    pub repetitiveness: f32,
    pub sentence_complexity: f32,
    pub question_tendency: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intention {
    Idle,
    Eat,
    Sleep,
    Play,
    ApproachPlayer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryKind {
    WasFed { food: FoodId },
    Played,
    WasComforted,
    PlayerReturnedAfterAbsence,
    DislikedFood { food: FoodId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub id: MemoryId,
    pub happened_at_ms: u64,
    pub kind: MemoryKind,
    pub concepts: BTreeSet<Concept>,
    pub valence: f32,
    pub salience: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub name: String,
    pub needs: Needs,
    pub traits: Traits,
    pub bond: f32,
    pub preferences: BTreeMap<FoodId, f32>,
    pub known_concepts: BTreeSet<Concept>,
    pub memories: Vec<Memory>,
    pub current_intention: Intention,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomState {
    pub food_in_bowl: Option<FoodId>,
    pub toy_available: bool,
    pub tidy: bool,
    pub player_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub save_version: u32,
    pub seed: u64,
    pub elapsed_ms: u64,
    pub next_memory_id: u64,
    pub creature: Creature,
    pub room: RoomState,
}

impl WorldState {
    #[must_use]
    pub fn new(seed: u64, name: impl Into<String>) -> Self {
        let mut genome = SeededRandom::new(seed ^ 0xa076_1d64_78bd_642f);
        Self {
            save_version: SAVE_VERSION,
            seed,
            elapsed_ms: 0,
            next_memory_id: 1,
            creature: Creature {
                name: name.into(),
                needs: Needs {
                    hunger: 0.25,
                    energy: 0.85,
                    comfort: 0.75,
                    curiosity: 0.55,
                },
                traits: Traits {
                    sociability: genome.next_unit(),
                    boldness: genome.next_unit(),
                    fussiness: genome.next_unit(),
                    stubbornness: genome.next_unit(),
                    literalness: genome.next_unit(),
                    repetitiveness: genome.next_unit(),
                    sentence_complexity: genome.next_unit(),
                    question_tendency: genome.next_unit(),
                },
                bond: 0.1,
                preferences: BTreeMap::new(),
                known_concepts: BTreeSet::from([
                    Concept::SelfIdentity,
                    Concept::You,
                    Concept::Food,
                    Concept::Good,
                    Concept::Bad,
                    Concept::Here,
                    Concept::Sleep,
                ]),
                memories: Vec::new(),
                current_intention: Intention::Idle,
            },
            room: RoomState {
                food_in_bowl: None,
                toy_available: true,
                tidy: true,
                player_present: true,
            },
        }
    }

    fn remember(&mut self, kind: MemoryKind, concepts: &[Concept], valence: f32, salience: f32) {
        let id = MemoryId(self.next_memory_id);
        self.next_memory_id += 1;
        self.creature.memories.push(Memory {
            id,
            happened_at_ms: self.elapsed_ms,
            kind,
            concepts: concepts.iter().copied().collect(),
            valence: valence.clamp(-1.0, 1.0),
            salience: salience.clamp(0.0, 1.0),
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Feed(FoodId),
    Play,
    Comfort,
    Tidy,
    ReturnedAfterAbsence,
    Talk,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    NeedChanged,
    MemoryCreated(MemoryId),
    FoodConsumed(FoodId),
    IntentionChanged { from: Intention, to: Intention },
}

pub trait RandomSource {
    fn next_unit(&mut self) -> f32;
}

#[derive(Debug, Clone)]
pub struct SeededRandom(u64);

impl SeededRandom {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl RandomSource for SeededRandom {
    fn next_unit(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.0 >> 40) as f32) / ((1_u32 << 24) as f32)
    }
}

pub fn step(
    state: &mut WorldState,
    input: &[PlayerEvent],
    dt_ms: u64,
    rng: &mut impl RandomSource,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    state.elapsed_ms = state.elapsed_ms.saturating_add(dt_ms);
    let minutes = dt_ms as f32 / 60_000.0;
    state.creature.needs.hunger += 0.025 * minutes;
    state.creature.needs.energy -= 0.018 * minutes;
    state.creature.needs.comfort -= 0.008 * minutes;
    state.creature.needs.curiosity += 0.012 * minutes;

    let previous_memory_count = state.creature.memories.len();
    enact_current_intention(state, minutes, rng, &mut events);
    for memory in &state.creature.memories[previous_memory_count..] {
        events.push(GameEvent::MemoryCreated(memory.id));
    }

    for event in input {
        let previous_memory_count = state.creature.memories.len();
        apply_player_event(state, event);
        for memory in &state.creature.memories[previous_memory_count..] {
            events.push(GameEvent::MemoryCreated(memory.id));
        }
    }

    state.creature.needs.clamp();
    state.creature.bond = state.creature.bond.clamp(0.0, 1.0);
    events.push(GameEvent::NeedChanged);

    let previous = state.creature.current_intention;
    let next = choose_intention(state, rng);
    if next != previous {
        state.creature.current_intention = next;
        events.push(GameEvent::IntentionChanged {
            from: previous,
            to: next,
        });
    }
    events
}

fn apply_player_event(state: &mut WorldState, event: &PlayerEvent) {
    match event {
        PlayerEvent::Feed(food) => state.room.food_in_bowl = Some(*food),
        PlayerEvent::Play => {
            state.creature.needs.curiosity -= 0.5;
            state.creature.bond += 0.06;
            state.creature.known_concepts.insert(Concept::Toy);
            state.remember(
                MemoryKind::Played,
                &[Concept::Toy, Concept::You, Concept::Good],
                0.65,
                0.7,
            );
        }
        PlayerEvent::Comfort => {
            state.creature.needs.comfort += 0.35;
            state.creature.bond += 0.08;
            state.remember(
                MemoryKind::WasComforted,
                &[Concept::You, Concept::Good],
                0.8,
                0.8,
            );
        }
        PlayerEvent::Tidy => state.room.tidy = true,
        PlayerEvent::ReturnedAfterAbsence => {
            state.room.player_present = true;
            state.remember(
                MemoryKind::PlayerReturnedAfterAbsence,
                &[Concept::You, Concept::Again],
                state.creature.bond,
                0.8,
            );
        }
        PlayerEvent::Talk => {}
    }
}

fn enact_current_intention(
    state: &mut WorldState,
    minutes: f32,
    rng: &mut impl RandomSource,
    events: &mut Vec<GameEvent>,
) {
    match state.creature.current_intention {
        Intention::Eat => {
            if let Some(food) = state.room.food_in_bowl.take() {
                let inherited = rng.next_unit() * 2.0 - 1.0;
                let preference = *state.creature.preferences.entry(food).or_insert(inherited);
                state.creature.needs.hunger -= 0.45;
                state.remember(
                    MemoryKind::WasFed { food },
                    &[Concept::Food, Concept::You],
                    preference,
                    0.75,
                );
                if preference < -0.35 {
                    state.remember(
                        MemoryKind::DislikedFood { food },
                        &[Concept::Food, Concept::Bad],
                        preference,
                        0.95,
                    );
                }
                events.push(GameEvent::FoodConsumed(food));
            }
        }
        Intention::Sleep => state.creature.needs.energy += 0.12 * minutes,
        Intention::Play => state.creature.needs.curiosity -= 0.06 * minutes,
        Intention::ApproachPlayer | Intention::Idle => {}
    }
}

fn choose_intention(state: &WorldState, rng: &mut impl RandomSource) -> Intention {
    let creature = &state.creature;
    let food_preference = state.room.food_in_bowl.map_or(0.5, |food| {
        creature
            .preferences
            .get(&food)
            .copied()
            .map_or(0.5, |value| (value + 1.0) / 2.0)
    });
    let candidates = [
        (
            Intention::Eat,
            creature.needs.hunger * f32::from(state.room.food_in_bowl.is_some()) * food_preference,
        ),
        (Intention::Sleep, (1.0 - creature.needs.energy) * 0.9),
        (
            Intention::Play,
            creature.needs.curiosity
                * f32::from(state.room.toy_available)
                * (0.6 + creature.traits.sociability * 0.4),
        ),
        (
            Intention::ApproachPlayer,
            f32::from(state.room.player_present)
                * creature.bond
                * (0.5 + creature.traits.sociability * 0.5),
        ),
        (Intention::Idle, 0.12),
    ];
    let current = creature.current_intention;
    candidates
        .into_iter()
        .map(|(intention, score)| {
            let hysteresis = if intention == current { 0.08 } else { 0.0 };
            let noise = rng.next_unit() * 0.025;
            (intention, score + hysteresis + noise)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(Intention::Idle, |(intention, _)| intention)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_seeds_and_inputs_replay_identically() {
        let mut first = WorldState::new(42, "Mop");
        let mut second = first.clone();
        let mut first_rng = SeededRandom::new(first.seed);
        let mut second_rng = SeededRandom::new(second.seed);
        let inputs = [PlayerEvent::Feed(FoodId::Berry), PlayerEvent::Play];

        assert_eq!(
            step(&mut first, &inputs, 60_000, &mut first_rng),
            step(&mut second, &inputs, 60_000, &mut second_rng)
        );
        assert_eq!(first, second);
    }

    #[test]
    fn needs_remain_bounded_over_many_days() {
        let mut world = WorldState::new(7, "Pip");
        let mut rng = SeededRandom::new(world.seed);
        for _ in 0..(24 * 60 * 10) {
            step(&mut world, &[], 60_000, &mut rng);
        }
        let needs = world.creature.needs;
        for value in [needs.hunger, needs.energy, needs.comfort, needs.curiosity] {
            assert!((0.0..=1.0).contains(&value));
            assert!(value.is_finite());
        }
    }

    #[test]
    fn an_unattended_creature_eventually_sleeps() {
        let mut world = WorldState::new(7, "Pip");
        let mut rng = SeededRandom::new(world.seed);
        let slept = (0..(24 * 60)).any(|_| {
            step(&mut world, &[], 60_000, &mut rng);
            world.creature.current_intention == Intention::Sleep
        });
        assert!(slept);
    }

    #[test]
    fn save_round_trip_preserves_identity_and_memories() {
        let mut world = WorldState::new(99, "Mrrp");
        let mut rng = SeededRandom::new(world.seed);
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut rng,
        );
        step(&mut world, &[], 1_000, &mut rng);
        let encoded = serde_json::to_string_pretty(&world).expect("world should serialize");
        let decoded: WorldState = serde_json::from_str(&encoded).expect("world should deserialize");
        assert_eq!(world, decoded);
    }

    #[test]
    fn feeding_stocks_the_bowl_and_eating_consumes_it() {
        let mut world = WorldState::new(99, "Mrrp");
        world.creature.needs.hunger = 1.0;
        let mut rng = SeededRandom::new(world.seed);

        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut rng,
        );
        assert_eq!(world.room.food_in_bowl, Some(FoodId::Berry));
        assert_eq!(world.creature.current_intention, Intention::Eat);

        let events = step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(world.room.food_in_bowl, None);
        assert!(events.contains(&GameEvent::FoodConsumed(FoodId::Berry)));
        assert!(world.creature.memories.iter().any(|memory| memory.kind
            == MemoryKind::WasFed {
                food: FoodId::Berry
            }));
    }

    #[test]
    fn eating_uses_the_preference_for_the_food_in_the_bowl() {
        let mut world = WorldState::new(99, "Mrrp");
        world.creature.needs.hunger = 0.8;
        world.creature.preferences.insert(FoodId::Berry, -1.0);
        world.creature.preferences.insert(FoodId::Mushroom, 1.0);
        let mut rng = SeededRandom::new(world.seed);

        world.room.food_in_bowl = Some(FoodId::Berry);
        step(&mut world, &[], 1_000, &mut rng);
        assert_ne!(world.creature.current_intention, Intention::Eat);

        world.room.food_in_bowl = Some(FoodId::Mushroom);
        step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::Eat);
    }
}
