//! Authoritative, deterministic creature simulation.

mod memory;
mod model;
mod random;
mod save;
mod simulation;

pub use memory::{MemoryCue, MemoryQuery, select_candidate_memories};
pub use model::{
    Belief, BeliefId, BeliefKind, Concept, Creature, Development, FoodId, Idiolect, IdiolectQuirk,
    Intention, InteractionCounters, LanguageStage, Memory, MemoryId, MemoryKind, Mood, Movement,
    Needs, NonverbalAct, Reaction, Relationship, RoomSpot, RoomState, SocialAct, SocialHabits,
    StateValidationError, ToyId, Traits, WorldState,
};
pub use random::{RandomSource, SeededRandom};
pub use save::{SaveError, SaveGame};
pub use simulation::{
    GameEvent, MAX_OFFLINE_MS, MOVEMENT_DURATION_MS, OfflineProgress, PlayerEvent,
    SIMULATION_TICK_MS, advance_offline, intention_target, step,
};

pub const SAVE_VERSION: u32 = 2;
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
        let inputs = [
            PlayerEvent::Feed(FoodId::Berry),
            PlayerEvent::Play(ToyId::Ball),
        ];

        assert_eq!(
            step(&mut first, &inputs, 60_000, &mut first_rng),
            step(&mut second, &inputs, 60_000, &mut second_rng)
        );
        assert_eq!(first, second);
        assert_eq!(first_rng, second_rng);
    }

    #[test]
    fn idiolect_is_plain_until_individuality_and_varies_by_identity() {
        let mut hatch = WorldState::new(1, "Mop");
        assert_eq!(hatch.idiolect().quirk, IdiolectQuirk::Plain);

        hatch.creature.development.active_days_reached = 3;
        hatch.creature.development.language_stage = LanguageStage::Phrases;
        hatch.creature.development.interactions.talks = 3;
        let first = hatch.idiolect();

        let mut variants = BTreeSet::new();
        for seed in 1..=16 {
            let mut world = WorldState::new(seed, "Mop");
            world.creature.development.active_days_reached = 3;
            world.creature.development.language_stage = LanguageStage::Phrases;
            world.creature.development.interactions.talks = 3;
            variants.insert(world.idiolect().quirk);
        }
        assert!(first.quirk != IdiolectQuirk::Plain);
        assert!(
            variants.len() >= 2,
            "seed projection should produce variants"
        );
    }

    #[test]
    fn idiolect_round_trips_with_the_persisted_identity() {
        let mut world = WorldState::new(99, "Mop");
        world.elapsed_ms = ACTIVE_DAY_MS * 2;
        world.creature.development.active_days_reached = 3;
        world.creature.development.language_stage = LanguageStage::Phrases;
        world.creature.development.interactions.talks = 3;
        world
            .creature
            .known_concepts
            .extend([Concept::Again, Concept::Yesterday]);
        let encoded = SaveGame::capture(&world, &SeededRandom::new(world.seed))
            .to_json()
            .expect("save should encode");
        let (reloaded, _) = SaveGame::from_json(&encoded)
            .expect("save should decode")
            .resume();
        assert_eq!(reloaded.idiolect(), world.idiolect());
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
        step(&mut world, &[], MOVEMENT_DURATION_MS, &mut rng);

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

        let play_events = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Ball)],
            MOVEMENT_DURATION_MS + SIMULATION_TICK_MS,
            &mut rng,
        );
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
        let rejection = step(&mut reloaded, &[], MOVEMENT_DURATION_MS, &mut reloaded_rng);
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

        let events = step(&mut world, &[], MOVEMENT_DURATION_MS, &mut rng);
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
    fn a_new_food_interrupts_idle_play_long_enough_to_be_tasted() {
        let mut world = WorldState::new(42, "Mop");
        let mut rng = SeededRandom::new(world.seed);
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Berry)],
            10_000,
            &mut rng,
        );

        assert_eq!(world.room.food_in_bowl, None);
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
        step(&mut world, &[], MOVEMENT_DURATION_MS, &mut rng);

        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Mushroom)],
            1_000,
            &mut rng,
        );
        assert_eq!(world.creature.current_intention, Intention::Eat);
    }

    #[test]
    fn invalid_save_versions_are_rejected() {
        let world = WorldState::new(42, "Mop");
        let random = SeededRandom::new(world.seed);
        let mut save = SaveGame::capture(&world, &random);
        save.save_version = 1;
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

        let future_inputs = [PlayerEvent::Play(ToyId::Ball), PlayerEvent::Comfort];
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

    #[test]
    fn one_minute_equals_sixty_one_second_ticks() {
        let mut batched = WorldState::new(123, "Tick");
        let mut incremental = batched.clone();
        batched.creature.needs.hunger = 1.0;
        incremental.creature.needs.hunger = 1.0;
        let mut batched_rng = SeededRandom::new(batched.seed);
        let mut incremental_rng = batched_rng;
        let input = [PlayerEvent::Feed(FoodId::Pellet)];

        let batched_events = step(&mut batched, &input, 60_000, &mut batched_rng);
        let mut incremental_events = Vec::new();
        for second in 0..60 {
            let events = if second == 0 {
                step(&mut incremental, &input, 1_000, &mut incremental_rng)
            } else {
                step(&mut incremental, &[], 1_000, &mut incremental_rng)
            };
            incremental_events.extend(events);
        }

        assert_eq!(batched, incremental);
        assert_eq!(batched_rng, incremental_rng);
        assert_eq!(batched_events, incremental_events);
        assert_eq!(batched.creature.development.interactions.feeds, 1);
    }

    #[test]
    fn fractional_tick_remainder_and_mid_movement_survive_save() {
        let mut world = WorldState::new(321, "Mover");
        world.creature.needs.hunger = 1.0;
        let mut rng = SeededRandom::new(world.seed);
        step(
            &mut world,
            &[PlayerEvent::Feed(FoodId::Pellet)],
            1_500,
            &mut rng,
        );
        assert_eq!(world.simulation_remainder_ms, 500);
        let movement = world.creature.movement.expect("moving to bowl");
        assert_eq!(movement.to, RoomSpot::Bowl);
        assert_eq!(movement.elapsed_ms, 0);

        let encoded = SaveGame::capture(&world, &rng)
            .to_json()
            .expect("mid-movement save");
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded)
            .expect("mid-movement load")
            .resume();
        let expected = step(&mut world, &[], 2_500, &mut rng);
        let actual = step(&mut resumed, &[], 2_500, &mut resumed_rng);
        assert_eq!(actual, expected);
        assert_eq!(resumed, world);
        assert_eq!(resumed_rng, rng);
        assert_eq!(resumed.creature.position, RoomSpot::Bowl);
        assert_eq!(resumed.room.food_in_bowl, None);
    }

    #[test]
    fn development_unlocks_are_grounded_monotonic_and_gate_adult_humor() {
        let mut world = WorldState::new(5, "Mouth");
        let mut rng = SeededRandom::new(world.seed);
        world.creature.social_habits.provocation = 0.0;
        world.creature.social_habits.spite = 0.0;
        world.creature.social_habits.profanity = 0.0;
        world.creature.social_habits.crudeness = 1.0;
        world.creature.social_habits.sexual_innuendo = 1.0;
        world.creature.relationship.resentment = 0.0;
        let hatch_talk = step(&mut world, &[PlayerEvent::Talk], 0, &mut rng);
        assert!(hatch_talk.contains(&GameEvent::SocialActExpressed(SocialAct::Neutral)));

        step(
            &mut world,
            &[
                PlayerEvent::Feed(FoodId::Pellet),
                PlayerEvent::Play(ToyId::Bell),
                PlayerEvent::Comfort,
                PlayerEvent::ReturnedAfterAbsence,
            ],
            0,
            &mut rng,
        );
        world.elapsed_ms = ACTIVE_DAY_MS * 2 - SIMULATION_TICK_MS;
        let milestone = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert_eq!(world.active_day(), 3);
        assert_eq!(
            world.creature.development.language_stage,
            LanguageStage::Phrases
        );
        for concept in [
            Concept::Again,
            Concept::Give,
            Concept::Toy,
            Concept::Yesterday,
            Concept::Trust,
            Concept::Friend,
            Concept::Why,
        ] {
            assert!(world.creature.known_concepts.contains(&concept));
        }
        assert!(milestone.contains(&GameEvent::LanguageAdvanced(LanguageStage::Words)));
        assert!(milestone.contains(&GameEvent::LanguageAdvanced(LanguageStage::Phrases)));

        let developed_talk = step(&mut world, &[PlayerEvent::Talk], 0, &mut rng);
        assert!(developed_talk.contains(&GameEvent::SocialActExpressed(SocialAct::Crudeness)));
        step(&mut world, &[], 10_000, &mut rng);
        assert_eq!(
            world.creature.development.language_stage,
            LanguageStage::Phrases
        );
        world.validate().expect("developed world is valid");
    }

    #[test]
    fn a_disliked_toy_is_rejected_only_after_arrival() {
        let mut world = WorldState::new(17, "Fuss");
        let mut rng = SeededRandom::new(world.seed);
        world.creature.toy_preferences.insert(ToyId::Sock, -1.0);
        let first = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Sock)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        assert!(!first.contains(&GameEvent::ToyRejected(ToyId::Sock)));
        assert_eq!(world.creature.position, RoomSpot::Center);
        let arrival = step(&mut world, &[], MOVEMENT_DURATION_MS, &mut rng);
        assert!(arrival.contains(&GameEvent::Arrived(RoomSpot::Toy)));
        assert!(arrival.contains(&GameEvent::ToyRejected(ToyId::Sock)));
        assert_eq!(
            world.room.last_nonverbal_act,
            Some(NonverbalAct::TakeToyAway(ToyId::Sock))
        );
        assert!(
            world
                .creature
                .memories
                .iter()
                .any(|memory| { memory.kind == MemoryKind::DislikedToy { toy: ToyId::Sock } })
        );
    }

    #[test]
    fn comfort_emits_an_authoritative_event_after_arrival() {
        let mut world = WorldState::new(73, "Mop");
        let mut rng = SeededRandom::new(world.seed);
        world.creature.needs.comfort = 0.0;

        let requested = step(&mut world, &[PlayerEvent::Comfort], 1_000, &mut rng);
        assert!(!requested.contains(&GameEvent::Comforted));
        assert_eq!(world.creature.current_intention, Intention::ApproachPlayer);

        let arrived = step(&mut world, &[], MOVEMENT_DURATION_MS, &mut rng);
        assert!(arrived.contains(&GameEvent::Comforted));
    }

    #[test]
    fn offline_progress_is_bounded_nonlethal_and_records_return() {
        let mut world = WorldState::new(71, "Homebody");
        let mut rng = SeededRandom::new(world.seed);
        let started_at = world.elapsed_ms;
        let progress = advance_offline(&mut world, MAX_OFFLINE_MS * 10, &mut rng);
        assert_eq!(progress.applied_ms, MAX_OFFLINE_MS);
        assert_eq!(world.elapsed_ms, started_at);
        assert_eq!(world.active_day(), 1);
        assert!(world.room.player_present);
        assert!(world.creature.needs.energy >= 0.2);
        assert!(world.creature.needs.comfort >= 0.2);
        assert!(world.creature.relationship.bond >= 0.05);
        assert!(
            world
                .creature
                .memories
                .iter()
                .any(|memory| { memory.kind == MemoryKind::PlayerReturnedAfterAbsence })
        );
        world.validate().expect("offline state is valid");
    }
}
