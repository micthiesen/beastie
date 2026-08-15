//! Authoritative, deterministic creature simulation.

mod memory;
mod model;
mod random;
mod save;
mod simulation;

pub use memory::{MemoryCue, MemoryQuery, select_candidate_memories};
pub use model::{
    Belief, BeliefId, BeliefKind, Concept, Creature, FoodId, Intention, Memory, MemoryId,
    MemoryKind, Needs, NonverbalAct, Reaction, Relationship, RoomState, SocialAct, SocialHabits,
    StateValidationError, Traits, WorldState,
};
pub use random::{RandomSource, SeededRandom};
pub use save::{SaveError, SaveGame};
pub use simulation::{GameEvent, PlayerEvent, step};

pub const SAVE_VERSION: u32 = 1;
pub const ACTIVE_DAY_MS: u64 = 15 * 60_000;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
        assert_eq!(first_rng, second_rng);
    }

    #[test]
    fn berry_memory_arc_survives_sleep_save_and_reload() {
        let mut world = WorldState::new(99, "Mrrp");
        let mut rng = SeededRandom::new(world.seed);
        world.creature.needs.hunger = 1.0;

        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut rng,
        );
        assert_eq!(world.creature.current_intention, Intention::Eat);
        step(&mut world, &[], 1_000, &mut rng);

        let berry_preference = world.creature.preferences[&FoodId::Berry];
        assert!(berry_preference < -0.35);
        assert!(world.creature.memories.iter().any(|memory| {
            memory.kind
                == MemoryKind::DislikedFood {
                    food: FoodId::Berry,
                }
        }));
        assert!(
            world
                .creature
                .beliefs
                .iter()
                .any(|belief| belief.kind == BeliefKind::RedFoodIsATrick)
        );

        let play_events = step(&mut world, &[PlayerEvent::Play], 1_000, &mut rng);
        world.creature.needs.energy = 0.0;
        let sleep_events = step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::Sleep);
        assert!(
            play_events.contains(&GameEvent::SleepStarted)
                || sleep_events.contains(&GameEvent::SleepStarted)
        );
        step(&mut world, &[], 60_000, &mut rng);
        assert!(world.creature.needs.energy > 0.0);

        let encoded = SaveGame::capture(&world, &rng)
            .to_json()
            .expect("save should encode");
        let (mut reloaded, mut reloaded_rng) = SaveGame::from_json(&encoded)
            .expect("save should decode")
            .resume();
        assert_eq!(reloaded, world);
        assert_eq!(reloaded_rng, rng);

        let candidates = select_candidate_memories(
            &reloaded,
            &MemoryQuery {
                cues: BTreeSet::from([
                    MemoryCue::Concept(Concept::Food),
                    MemoryCue::Concept(Concept::Bad),
                    MemoryCue::Food(FoodId::Berry),
                ]),
                limit: 8,
            },
        );
        assert!(matches!(
            candidates.first().map(|memory| &memory.kind),
            Some(MemoryKind::DislikedFood {
                food: FoodId::Berry
            })
        ));

        let first_talk = step(
            &mut reloaded,
            &[PlayerEvent::Talk],
            1_000,
            &mut reloaded_rng,
        );
        assert!(first_talk.contains(&GameEvent::SocialActExpressed(SocialAct::Insult)));
        step(
            &mut reloaded,
            &[PlayerEvent::React(Reaction::Laugh)],
            1_000,
            &mut reloaded_rng,
        );
        let second_talk = step(
            &mut reloaded,
            &[PlayerEvent::Talk],
            1_000,
            &mut reloaded_rng,
        );
        assert!(second_talk.contains(&GameEvent::SocialActExpressed(SocialAct::Provocation)));

        reloaded.creature.needs.hunger = 1.0;
        reloaded.creature.needs.energy = 1.0;
        reloaded.creature.needs.curiosity = 0.0;
        step(
            &mut reloaded,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut reloaded_rng,
        );
        assert_eq!(reloaded.creature.current_intention, Intention::RejectFood);
        let rejection = step(&mut reloaded, &[], 1_000, &mut reloaded_rng);
        assert!(rejection.contains(&GameEvent::FoodRejected(FoodId::Berry)));
        assert_eq!(
            reloaded.room.last_nonverbal_act,
            Some(NonverbalAct::PushFoodAway(FoodId::Berry))
        );
        step(&mut reloaded, &[], 1_000, &mut reloaded_rng);
        assert_eq!(
            reloaded.room.last_nonverbal_act,
            Some(NonverbalAct::PushFoodAway(FoodId::Berry))
        );
        reloaded.validate().expect("scenario state should be valid");
    }

    #[test]
    fn needs_and_references_remain_valid_over_many_days() {
        let mut world = WorldState::new(7, "Pip");
        let mut rng = SeededRandom::new(world.seed);
        for minute in 0..(24 * 60 * 30) {
            let input = if minute % 360 == 0 {
                vec![PlayerEvent::Feed(FoodId::Pellet)]
            } else {
                Vec::new()
            };
            step(&mut world, &input, 60_000, &mut rng);
            world.validate().expect("long simulation should stay valid");
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
        assert!(world.creature.memories.iter().any(|memory| {
            memory.kind
                == MemoryKind::WasFed {
                    food: FoodId::Berry,
                }
        }));
    }

    #[test]
    fn a_known_hated_food_is_rejected_instead_of_eaten() {
        let mut world = WorldState::new(99, "Mrrp");
        world.creature.needs.hunger = 1.0;
        world.creature.preferences.insert(FoodId::Berry, -1.0);
        world.creature.preferences.insert(FoodId::Mushroom, 1.0);
        let mut rng = SeededRandom::new(world.seed);

        world.room.food_in_bowl = Some(FoodId::Berry);
        step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::RejectFood);
        step(&mut world, &[], 1_000, &mut rng);

        world.room.food_in_bowl = Some(FoodId::Mushroom);
        step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::Eat);
    }

    #[test]
    fn invalid_save_versions_are_rejected() {
        let world = WorldState::new(42, "Mop");
        let random = SeededRandom::new(world.seed);
        let mut save = SaveGame::capture(&world, &random);
        save.save_version = SAVE_VERSION + 1;
        assert!(matches!(save.to_json(), Err(SaveError::Version(_))));
    }

    #[test]
    fn invalid_traits_and_zero_next_ids_are_rejected() {
        let mut world = WorldState::new(42, "Mop");
        world.creature.traits.stubbornness = 1.1;
        assert_eq!(world.validate(), Err(StateValidationError::TraitScalar));

        world.creature.traits.stubbornness = 0.5;
        world.next_memory_id = 0;
        assert_eq!(world.validate(), Err(StateValidationError::MemoryOrder));

        world.next_memory_id = 1;
        world.next_belief_id = 0;
        assert_eq!(world.validate(), Err(StateValidationError::BeliefId));
    }

    #[test]
    fn save_reload_preserves_future_simulation() {
        let mut uninterrupted = WorldState::new(99, "Mrrp");
        let mut uninterrupted_rng = SeededRandom::new(uninterrupted.seed);
        step(
            &mut uninterrupted,
            &[PlayerEvent::Feed(FoodId::Berry)],
            1_000,
            &mut uninterrupted_rng,
        );
        let encoded = SaveGame::capture(&uninterrupted, &uninterrupted_rng)
            .to_json()
            .expect("checkpoint should encode");
        let (mut reloaded, mut reloaded_rng) = SaveGame::from_json(&encoded)
            .expect("checkpoint should decode")
            .resume();

        let future_inputs = [PlayerEvent::Play, PlayerEvent::Comfort];
        let uninterrupted_events = step(
            &mut uninterrupted,
            &future_inputs,
            60_000,
            &mut uninterrupted_rng,
        );
        let reloaded_events = step(&mut reloaded, &future_inputs, 60_000, &mut reloaded_rng);
        assert_eq!(reloaded_events, uninterrupted_events);
        assert_eq!(reloaded, uninterrupted);
        assert_eq!(reloaded_rng, uninterrupted_rng);
    }
}
