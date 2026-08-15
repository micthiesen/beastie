//! Authoritative, deterministic creature simulation.

mod memory;
mod model;
mod random;
mod save;
mod simulation;

pub use memory::{MemoryCue, MemoryQuery, select_candidate_memories};
pub use model::{
    ActionPhase, ActionTimeline, AquariumCreatureState, AquariumPosition, AquariumState, Belief,
    BeliefId, BeliefKind, Concept, ConversationState, Creature, DepthLane, Development,
    DevelopmentMilestone, Facing, FoodBuoyancy, FoodDisposition, FoodDropRejectionReason, FoodId,
    FoodObject, GazeTarget, Idiolect, IdiolectQuirk, InitiatedBehavior, InitiativeReason,
    Intention, InteractionCounters, LanguageExposure, LanguageStage, Memory, MemoryId, MemoryKind,
    Mood, NamingTarget, Needs, NonverbalAct, NormalizedPosition, NormalizedVelocity, Reaction,
    Relationship, Routine, SemanticDestination, SocialAct, SocialHabits, StateValidationError,
    SteeringMode, ToyId, Traits, WorldObject, WorldState,
};
pub use random::{RandomDomain, RandomSource, SeededRandom, deterministic_unit};
pub use save::{SaveError, SaveGame};
pub use simulation::{
    GameEvent, MAX_OFFLINE_MS, OfflineProgress, PlayerEvent, SIMULATION_TICK_MS, SpeechAttention,
    TALK_COOLDOWN_MS, advance_offline, step,
};

pub const SAVE_VERSION: u32 = 3;
pub const ACTIVE_DAY_MS: u64 = 15 * 60_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aquarium_feeding_has_fixed_phases_and_bounded_position() {
        let mut world = WorldState::new(7, "Swim");
        let mut rng = SeededRandom::new(7);
        let events = step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(-5, 20_005),
            }],
            0,
            &mut rng,
        );
        assert!(events.contains(&GameEvent::ActionPhaseChanged {
            from: None,
            to: ActionPhase::Notice
        }));
        let mut phases = Vec::new();
        for _ in 0..20 {
            phases.extend(
                step(&mut world, &[], 1_000, &mut rng)
                    .into_iter()
                    .filter_map(|event| match event {
                        GameEvent::ActionPhaseChanged { to, .. } => Some(to),
                        _ => None,
                    }),
            );
        }
        for phase in [
            ActionPhase::Brake,
            ActionPhase::Gaze,
            ActionPhase::Turn,
            ActionPhase::Approach,
            ActionPhase::Inspect,
            ActionPhase::Act,
            ActionPhase::Recover,
        ] {
            assert!(phases.contains(&phase));
        }
        assert!(world.creature.aquarium.action.is_none());
        assert!(
            world.creature.aquarium.position.x >= 0
                && world.creature.aquarium.position.x <= NormalizedPosition::SCALE
        );
        assert!(
            world.creature.aquarium.position.y >= 0
                && world.creature.aquarium.position.y <= NormalizedPosition::SCALE
        );
    }

    #[test]
    fn split_ticks_and_domain_draws_are_replay_stable() {
        let mut first = WorldState::new(11, "Exact");
        let mut second = first.clone();
        let mut first_rng = SeededRandom::new(11);
        let mut second_rng = SeededRandom::new(11);
        step(
            &mut first,
            &[PlayerEvent::DropFood {
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(2_000, 3_000),
            }],
            4_000,
            &mut first_rng,
        );
        step(
            &mut second,
            &[PlayerEvent::DropFood {
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(2_000, 3_000),
            }],
            1_000,
            &mut second_rng,
        );
        for _ in 0..3 {
            step(&mut second, &[], 1_000, &mut second_rng);
        }
        assert_eq!(first, second);
        let _draw = first.domain_draw(RandomDomain::Social);
        let encoded = SaveGame::capture(&first, &first_rng)
            .to_json()
            .expect("save");
        let expected_next = first.domain_draw(RandomDomain::Social);
        let (mut reloaded, _) = SaveGame::from_json(&encoded).expect("reload").resume();
        assert_eq!(expected_next, reloaded.domain_draw(RandomDomain::Social));
    }

    #[test]
    fn identical_seeds_and_inputs_replay_identically() {
        let mut a = WorldState::new(42, "Mop");
        let mut b = a.clone();
        let mut ar = SeededRandom::new(42);
        let mut br = SeededRandom::new(42);
        let input = [PlayerEvent::DropFood {
            food: FoodId::Pellet,
            position: NormalizedPosition::new(4_000, 3_000),
        }];
        assert_eq!(
            step(&mut a, &input, 8_000, &mut ar),
            step(&mut b, &input, 8_000, &mut br)
        );
        assert_eq!(a, b);
        assert_eq!(ar, br);
    }

    #[test]
    fn speech_is_perceived_before_words_without_interrupting_an_action() {
        let mut world = WorldState::new(42, "Listener");
        world.creature.traits.sociability = 1.0;
        world.creature.relationship.bond = 1.0;
        world.creature.current_intention = Intention::Eat;
        world.creature.aquarium.action = Some(ActionTimeline {
            phase: ActionPhase::Act,
            elapsed_ms: 120,
            phase_duration_ms: 800,
            destination: SemanticDestination::Food(4),
            food_id: Some(4),
        });
        let action = world.creature.aquarium.action;
        let intention = world.creature.current_intention;
        let steering = world.creature.aquarium.steering;
        let destination = world.creature.aquarium.destination;
        let mut rng = SeededRandom::new(42);

        let events = step(&mut world, &[PlayerEvent::SpeechStarted], 0, &mut rng);

        assert_eq!(
            events,
            vec![GameEvent::SpeechPerceived(SpeechAttention::Glanced)]
        );
        assert_eq!(world.creature.aquarium.gaze, GazeTarget::Player);
        assert_eq!(world.creature.aquarium.action, action);
        assert_eq!(world.creature.current_intention, intention);
        assert_eq!(world.creature.aquarium.steering, steering);
        assert_eq!(world.creature.aquarium.destination, destination);
        assert!(world.creature.memories.is_empty());
    }

    #[test]
    fn speech_attention_is_deterministic_and_can_be_ignored() {
        let mut first = WorldState::new(9, "Sleeper");
        first.creature.current_intention = Intention::Sleep;
        let mut second = first.clone();
        let mut first_rng = SeededRandom::new(9);
        let mut second_rng = first_rng;

        let expected = step(&mut first, &[PlayerEvent::SpeechStarted], 0, &mut first_rng);
        let actual = step(
            &mut second,
            &[PlayerEvent::SpeechStarted],
            0,
            &mut second_rng,
        );

        assert_eq!(
            expected,
            vec![GameEvent::SpeechPerceived(SpeechAttention::Ignored)]
        );
        assert_eq!(actual, expected);
        assert_eq!(second, first);
        assert_eq!(second_rng, first_rng);
    }

    #[test]
    fn idiolect_is_plain_until_individuality_and_varies_by_identity() {
        let mut a = WorldState::new(1, "Mop");
        assert_eq!(a.idiolect().quirk, IdiolectQuirk::Plain);
        a.creature.development.active_days_reached = 3;
        a.creature.development.language_stage = LanguageStage::Phrases;
        a.creature.development.interactions.talks = 3;
        a.creature
            .known_concepts
            .extend([Concept::Again, Concept::Yesterday]);
        assert_ne!(a.idiolect().quirk, IdiolectQuirk::Plain);
    }

    #[test]
    fn idiolect_round_trips_with_the_persisted_identity() {
        let mut a = WorldState::new(99, "Mop");
        a.elapsed_ms = ACTIVE_DAY_MS * 2;
        a.creature.development.active_days_reached = 3;
        a.creature.development.language_stage = LanguageStage::Phrases;
        a.creature.development.interactions.talks = 3;
        a.creature
            .known_concepts
            .extend([Concept::Again, Concept::Yesterday]);
        let json = SaveGame::capture(&a, &SeededRandom::new(a.seed))
            .to_json()
            .unwrap();
        let (b, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(a.idiolect(), b.idiolect());
    }

    #[test]
    fn berry_memory_arc_survives_sleep_save_and_reload() {
        let mut a = WorldState::new(99, "Mrrp");
        let mut rng = SeededRandom::new(99);
        step(
            &mut a,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(4_000, 3_000),
            }],
            0,
            &mut rng,
        );
        step(&mut a, &[], 20_000, &mut rng);
        assert!(a.creature.memories.iter().any(|m| matches!(
            m.kind,
            MemoryKind::WasFed {
                food: FoodId::Berry
            } | MemoryKind::RejectedFood {
                food: FoodId::Berry
            }
        )));
        let json = SaveGame::capture(&a, &rng).to_json().unwrap();
        let (b, br) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(a, b);
        assert_eq!(rng, br);
    }

    #[test]
    fn needs_and_references_remain_valid_over_many_days() {
        let mut a = WorldState::new(7, "Pip");
        let mut rng = SeededRandom::new(7);
        for _ in 0..2_000 {
            step(&mut a, &[], 1_000, &mut rng);
            a.validate().unwrap();
        }
    }

    #[test]
    fn an_unattended_creature_eventually_sleeps() {
        let mut a = WorldState::new(7, "Pip");
        a.creature.needs.energy = 0.05;
        let mut rng = SeededRandom::new(7);
        let events = step(&mut a, &[], 1_000, &mut rng);
        assert!(events.contains(&GameEvent::SleepStarted));
    }

    #[test]
    fn feeding_stocks_the_aquarium_and_eating_consumes_it() {
        let mut a = WorldState::new(99, "Mrrp");
        let mut rng = SeededRandom::new(99);
        step(
            &mut a,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(4_000, 3_000),
            }],
            0,
            &mut rng,
        );
        assert_eq!(
            a.aquarium
                .objects
                .values()
                .filter(|object| matches!(object, WorldObject::Food(_)))
                .count(),
            1
        );
        let events = step(&mut a, &[], 20_000, &mut rng);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::FoodConsumed(_) | GameEvent::FoodRejected(_)))
        );
    }

    #[test]
    fn a_new_food_interrupts_idle_play_long_enough_to_be_tasted() {
        let mut a = WorldState::new(1, "Mop");
        let mut rng = SeededRandom::new(1);
        step(
            &mut a,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(5_000, 5_000),
            }],
            0,
            &mut rng,
        );
        assert_eq!(
            a.creature.aquarium.action.unwrap().phase,
            ActionPhase::Notice
        );
    }

    #[test]
    fn a_known_hated_food_is_rejected_instead_of_eaten() {
        let mut a = WorldState::new(1, "Mop");
        a.creature.preferences.insert(FoodId::Berry, -1.0);
        let mut rng = SeededRandom::new(1);
        step(
            &mut a,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 5_000),
            }],
            8_000,
            &mut rng,
        );
        assert!(a.creature.memories.iter().any(|m| matches!(
            m.kind,
            MemoryKind::RejectedFood {
                food: FoodId::Berry
            }
        )));
    }

    #[test]
    fn invalid_save_versions_are_rejected() {
        let a = WorldState::new(1, "Strict");
        let mut save = SaveGame::capture(&a, &SeededRandom::new(1));
        save.save_version = 2;
        assert!(matches!(save.to_json(), Err(SaveError::Version(2))));
    }

    #[test]
    fn invalid_traits_and_zero_next_ids_are_rejected() {
        let mut a = WorldState::new(1, "Strict");
        a.next_memory_id = 0;
        assert!(matches!(
            a.validate(),
            Err(StateValidationError::MemoryOrder)
        ));
    }

    #[test]
    fn save_reload_preserves_future_simulation() {
        let mut a = WorldState::new(22, "Save");
        let mut rng = SeededRandom::new(22);
        step(&mut a, &[], 2_500, &mut rng);
        let json = SaveGame::capture(&a, &rng).to_json().unwrap();
        let (mut b, mut br) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(
            step(&mut a, &[], 4_000, &mut rng),
            step(&mut b, &[], 4_000, &mut br)
        );
        assert_eq!(a, b);
    }

    #[test]
    fn one_minute_equals_sixty_one_second_ticks() {
        let mut a = WorldState::new(3, "Ticks");
        let mut b = a.clone();
        let mut ar = SeededRandom::new(3);
        let mut br = SeededRandom::new(3);
        step(&mut a, &[], 60_000, &mut ar);
        for _ in 0..60 {
            step(&mut b, &[], 1_000, &mut br);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn fractional_tick_remainder_and_mid_action_survive_save() {
        let mut a = WorldState::new(4, "Fraction");
        let mut rng = SeededRandom::new(4);
        step(
            &mut a,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(3_000, 3_000),
            }],
            1_500,
            &mut rng,
        );
        assert_eq!(a.simulation_remainder_ms, 500);
        let json = SaveGame::capture(&a, &rng).to_json().unwrap();
        let (b, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(a, b);
    }

    #[test]
    fn development_unlocks_are_grounded_monotonic_and_gate_adult_humor() {
        let mut a = WorldState::new(5, "Dev");
        a.elapsed_ms = ACTIVE_DAY_MS * 4;
        a.creature.development.interactions.feeds = 1;
        let mut rng = SeededRandom::new(5);
        step(&mut a, &[], 1_000, &mut rng);
        assert!(a.creature.known_concepts.contains(&Concept::Again));
        assert!(
            a.creature
                .development
                .milestones
                .contains(&DevelopmentMilestone::SettledRoutine)
        );
    }

    #[test]
    fn toy_preferences_and_routines_persist() {
        let mut a = WorldState::new(6, "Habits");
        a.set_routine(Routine {
            hour_start: 8,
            destination: SemanticDestination::Plant,
            strength: 3,
        });
        a.record_favorite(SemanticDestination::Cave);
        let json = SaveGame::capture(&a, &SeededRandom::new(6))
            .to_json()
            .unwrap();
        let (b, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(a.creature.routines, b.creature.routines);
        assert_eq!(a.favorite_destination(), b.favorite_destination());
    }

    #[test]
    fn comfort_emits_an_authoritative_event() {
        let mut a = WorldState::new(7, "Comfort");
        let mut rng = SeededRandom::new(7);
        let events = step(&mut a, &[PlayerEvent::Comfort], 0, &mut rng);
        assert!(events.contains(&GameEvent::Comforted));
    }

    #[test]
    fn offline_progress_is_bounded_nonlethal_and_records_return() {
        let mut a = WorldState::new(8, "Away");
        let mut rng = SeededRandom::new(8);
        let progress = advance_offline(&mut a, u64::MAX, &mut rng);
        assert_eq!(progress.applied_ms, MAX_OFFLINE_MS);
        assert!(a.creature.needs.energy >= 0.2 && a.creature.needs.comfort >= 0.2);
        assert!(a.aquarium.player_present);
    }

    #[test]
    fn each_need_selects_a_distinct_visible_state() {
        let mut a = WorldState::new(9, "Needs");
        a.creature.needs.hunger = 0.9;
        assert_eq!(a.mood(), Mood::Hungry);
        a.creature.needs.hunger = 0.1;
        a.creature.needs.energy = 0.1;
        assert_eq!(a.mood(), Mood::Sleepy);
    }

    #[test]
    fn talk_is_scarce_but_allows_one_reaction_follow_up() {
        let mut a = WorldState::new(10, "Talk");
        let mut rng = SeededRandom::new(10);
        assert!(step(&mut a, &[PlayerEvent::Talk], 0, &mut rng).contains(
            &GameEvent::TalkAccepted {
                contextual_follow_up: false
            }
        ));
        assert!(step(&mut a, &[PlayerEvent::Talk], 0, &mut rng).contains(&GameEvent::TalkIgnored));
        step(&mut a, &[PlayerEvent::React(Reaction::Laugh)], 0, &mut rng);
        assert!(step(&mut a, &[PlayerEvent::Talk], 0, &mut rng).contains(
            &GameEvent::TalkAccepted {
                contextual_follow_up: true
            }
        ));
    }

    #[test]
    fn typed_language_exposure_makes_permitted_habits_reachable() {
        let mut a = WorldState::new(11, "Words");
        let mut rng = SeededRandom::new(11);
        step(
            &mut a,
            &[PlayerEvent::LanguageExposure(LanguageExposure::Profanity)],
            0,
            &mut rng,
        );
        assert!(a.creature.social_habits.profanity > 0.02);
    }

    #[test]
    fn motivated_creature_initiates_nonverbal_request() {
        let mut a = WorldState::new(12, "Ask");
        a.creature.needs.comfort = 0.1;
        a.creature.traits.sociability = 1.0;
        let mut rng = SeededRandom::new(12);
        let events = step(&mut a, &[], 1_000, &mut rng);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::NonverbalRequest(_)))
        );
    }

    #[test]
    fn positive_interactions_slowly_recover_resentment() {
        let mut a = WorldState::new(13, "Mend");
        a.creature.relationship.resentment = 0.8;
        let before = a.creature.relationship.resentment;
        let mut rng = SeededRandom::new(13);
        step(&mut a, &[PlayerEvent::Comfort], 0, &mut rng);
        assert!(a.creature.needs.comfort > 0.75 || before == a.creature.relationship.resentment);
    }

    #[test]
    fn v1_fixture_migrates_to_aquarium_without_legacy_runtime_fields() {
        let legacy = include_str!("../../../fixtures/saves/v1-berry-ball.json");
        let (a, _) = SaveGame::from_json(legacy).unwrap().resume();
        assert_eq!(a.save_version, SAVE_VERSION);
        assert_eq!(a.creature.aquarium.action, None);
        assert!(a.aquarium.player_present);
    }

    #[test]
    fn v2_save_migrates_without_legacy_runtime_fields() {
        let a = WorldState::new(77, "V2");
        let mut value =
            serde_json::to_value(SaveGame::capture(&a, &SeededRandom::new(77))).unwrap();
        value["save_version"] = 2.into();
        value["world"]["save_version"] = 2.into();
        let (migrated, _) = SaveGame::from_json(&value.to_string()).unwrap().resume();
        assert_eq!(migrated.save_version, SAVE_VERSION);
        assert!(migrated.validate().is_ok());
    }

    #[test]
    fn cursor_is_persisted_and_uses_bounded_follow_or_flee_intent() {
        let mut world = WorldState::new(88, "Cursor");
        let mut rng = SeededRandom::new(88);
        world.creature.relationship.trust = 0.8;
        world.creature.traits.sociability = 0.8;
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(NormalizedPosition::new(
                12_000, -4,
            )))],
            0,
            &mut rng,
        );
        assert_eq!(
            world.aquarium.cursor,
            Some(NormalizedPosition::new(10_000, 0))
        );
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Approach);
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Position(NormalizedPosition::new(
                10_000, 0
            )))
        );
        world.creature.relationship.resentment = 0.9;
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(NormalizedPosition::new(
                2_000, 2_000,
            )))],
            0,
            &mut rng,
        );
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Flee);
    }

    #[test]
    fn physical_drop_counts_feeds_once_and_rejected_cap_does_not_count() {
        let mut world = WorldState::new(89, "Count");
        world.aquarium.max_food = 1;
        let mut rng = SeededRandom::new(89);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(4_000, 3_000),
            }],
            0,
            &mut rng,
        );
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(4_000, 3_000),
            }],
            0,
            &mut rng,
        );
        assert_eq!(world.creature.development.interactions.feeds, 1);
    }

    #[test]
    fn default_aquarium_objects_have_stable_ids_and_positions() {
        let aquarium = AquariumState::default();
        assert_eq!(aquarium.next_object_id, 6);
        assert!(matches!(
            aquarium.objects.get(&1),
            Some(WorldObject::Cave { .. })
        ));
        assert!(matches!(
            aquarium.objects.get(&2),
            Some(WorldObject::Plant { .. })
        ));
        assert!(matches!(
            aquarium.objects.get(&3),
            Some(WorldObject::Toy {
                toy: ToyId::Ball,
                ..
            })
        ));
        assert!(matches!(
            aquarium.objects.get(&4),
            Some(WorldObject::Toy {
                toy: ToyId::Bell,
                ..
            })
        ));
        assert!(matches!(
            aquarium.objects.get(&5),
            Some(WorldObject::Toy {
                toy: ToyId::Sock,
                ..
            })
        ));
    }

    #[test]
    fn full_aquarium_rejects_drop_without_mutation() {
        let mut state = WorldState::new(12, "Pip");
        state.aquarium.max_food = 0;
        let before = state.clone();
        let mut rng = SeededRandom::new(12);
        let events = step(
            &mut state,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 2_000),
            }],
            0,
            &mut rng,
        );
        assert_eq!(state.aquarium.objects, before.aquarium.objects);
        assert_eq!(
            state.aquarium.next_object_id,
            before.aquarium.next_object_id
        );
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::FoodDropRejected(FoodDropRejectionReason::AquariumFull)
        )));
    }

    #[test]
    fn food_approach_reduces_distance_before_any_resolution() {
        let mut world = WorldState::new(90, "Swim");
        let mut rng = SeededRandom::new(90);
        let events = step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(8_500, 2_000),
            }],
            0,
            &mut rng,
        );
        let food_id = events
            .iter()
            .find_map(|event| match event {
                GameEvent::FoodDropped { id, .. } => Some(*id),
                _ => None,
            })
            .expect("food should be dropped");
        let distance = |world: &WorldState| {
            let WorldObject::Food(food) = world.aquarium.objects[&food_id] else {
                panic!("food must remain before resolution")
            };
            (world.creature.aquarium.position.x - food.position.x).abs()
                + (world.creature.aquarium.position.y - food.position.y).abs()
        };
        let initial_distance = distance(&world);
        for _ in 0..7 {
            let events = step(&mut world, &[], 1_000, &mut rng);
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::FoodConsumed(_) | GameEvent::FoodRejected(_)
            )));
        }
        assert!(distance(&world) < initial_distance);
        assert!(world.creature.aquarium.action.is_some());
    }

    #[test]
    fn cursor_follow_and_flee_move_until_their_stop_thresholds() {
        let mut world = WorldState::new(91, "Cursor");
        let mut rng = SeededRandom::new(91);
        world.creature.relationship.trust = 0.9;
        world.creature.traits.sociability = 0.9;
        let cursor = NormalizedPosition::new(8_500, 4_500);
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(cursor))],
            0,
            &mut rng,
        );
        let before_follow = (world.creature.aquarium.position.x - cursor.x).abs();
        step(&mut world, &[], 1_000, &mut rng);
        assert!(world.creature.aquarium.position.x > 5_000);
        assert!((world.creature.aquarium.position.x - cursor.x).abs() < before_follow);

        world.creature.relationship.resentment = 0.9;
        let flee_cursor = NormalizedPosition::new(6_000, 4_500);
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(flee_cursor))],
            0,
            &mut rng,
        );
        let before_flee = (world.creature.aquarium.position.x - flee_cursor.x).abs();
        for _ in 0..3 {
            step(&mut world, &[], 1_000, &mut rng);
        }
        assert!((world.creature.aquarium.position.x - flee_cursor.x).abs() > before_flee);
        for _ in 0..8 {
            step(&mut world, &[], 1_000, &mut rng);
        }
        assert_ne!(world.creature.aquarium.steering, SteeringMode::Flee);
    }

    #[test]
    fn needs_traits_routines_and_favorites_choose_distinct_destinations() {
        let mut hungry = WorldState::new(92, "Hungry");
        hungry.creature.needs.hunger = 0.9;
        let mut rng = SeededRandom::new(92);
        step(&mut hungry, &[], 1_000, &mut rng);
        assert_eq!(
            hungry.creature.aquarium.destination,
            Some(SemanticDestination::Bottom)
        );

        let mut fussy = WorldState::new(93, "Fussy");
        fussy.creature.needs.curiosity = 0.9;
        fussy.creature.traits.fussiness = 0.9;
        let mut fussy_rng = SeededRandom::new(93);
        step(&mut fussy, &[], 1_000, &mut fussy_rng);
        assert_eq!(
            fussy.creature.aquarium.destination,
            Some(SemanticDestination::Plant)
        );

        let mut routine = WorldState::new(94, "Routine");
        routine.set_routine(Routine {
            hour_start: 0,
            destination: SemanticDestination::Cave,
            strength: 3,
        });
        let mut routine_rng = SeededRandom::new(94);
        step(&mut routine, &[], 1_000, &mut routine_rng);
        assert_eq!(
            routine.creature.aquarium.destination,
            Some(SemanticDestination::Cave)
        );
        routine.creature.routines.clear();
        routine.record_favorite(SemanticDestination::Plant);
        routine.elapsed_ms = 7_000;
        routine.creature.aquarium.destination = None;
        step(&mut routine, &[], 1_000, &mut routine_rng);
        assert_eq!(
            routine.creature.aquarium.destination,
            Some(SemanticDestination::Plant)
        );
    }

    #[test]
    fn active_play_is_not_interrupted_by_periodic_favorite_visit() {
        let mut world = WorldState::new(96, "Focused");
        world.creature.needs.curiosity = 0.9;
        world.creature.current_intention = Intention::Play;
        world.creature.aquarium.position = NormalizedPosition::new(5_000, 8_900);
        world.creature.aquarium.destination = None;
        world.record_favorite(SemanticDestination::Cave);
        world.elapsed_ms = 7_000;
        let mut rng = SeededRandom::new(96);

        let events = step(&mut world, &[], 1_000, &mut rng);

        assert_eq!(world.creature.current_intention, Intention::Play);
        assert!(!events.iter().any(|event| matches!(
            event,
            GameEvent::IntentionChanged {
                from: Intention::Play,
                to: Intention::Idle
            }
        )));
    }

    #[test]
    fn play_tidy_and_reunion_change_authoritative_aquarium_behavior() {
        let mut world = WorldState::new(95, "Actions");
        let mut rng = SeededRandom::new(95);
        world.creature.toy_preferences.insert(ToyId::Bell, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Bell)], 0, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::Play);
        assert_eq!(world.creature.aquarium.gaze, GazeTarget::Toy(ToyId::Bell));
        assert!(
            world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Bell }))
        );
        world.creature.toy_preferences.insert(ToyId::Sock, -1.0);
        let rejected_toy = step(&mut world, &[PlayerEvent::Play(ToyId::Sock)], 0, &mut rng);
        assert!(rejected_toy.contains(&GameEvent::ToyRejected(ToyId::Sock)));
        assert!(
            world
                .creature
                .beliefs
                .iter()
                .any(|belief| belief.kind == BeliefKind::ToyIsJealous)
        );

        world.creature.preferences.insert(FoodId::Berry, -1.0);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_500, 4_500),
            }],
            20_000,
            &mut rng,
        );
        assert!(world.aquarium.objects.values().any(|object| matches!(
            object,
            WorldObject::Food(food) if food.disposition == FoodDisposition::Rejected
        )));
        step(&mut world, &[PlayerEvent::Tidy], 0, &mut rng);
        assert!(!world.aquarium.objects.values().any(|object| matches!(
            object,
            WorldObject::Food(food) if food.disposition == FoodDisposition::Rejected
        )));

        let progress = advance_offline(&mut world, ACTIVE_DAY_MS, &mut rng);
        assert!(
            progress
                .events
                .contains(&GameEvent::NonverbalAct(NonverbalAct::LeanAgainstPlayer))
        );
        assert_eq!(world.creature.current_intention, Intention::ApproachPlayer);
        assert!(
            world
                .creature
                .beliefs
                .iter()
                .any(|belief| { belief.kind == BeliefKind::PlayerReturnsAfterSleep })
        );
    }

    #[test]
    fn mid_approach_save_resume_preserves_the_exact_future() {
        let mut first = WorldState::new(96, "Resume");
        let mut first_rng = SeededRandom::new(96);
        step(
            &mut first,
            &[PlayerEvent::DropFood {
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(8_000, 2_000),
            }],
            6_000,
            &mut first_rng,
        );
        assert_eq!(
            first.creature.aquarium.action.map(|action| action.phase),
            Some(ActionPhase::Approach)
        );
        let encoded = SaveGame::capture(&first, &first_rng)
            .to_json()
            .expect("save mid approach");
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded).expect("load").resume();
        let expected = step(&mut first, &[], 10_000, &mut first_rng);
        let actual = step(&mut resumed, &[], 10_000, &mut resumed_rng);
        assert_eq!(actual, expected);
        assert_eq!(resumed, first);
    }
}
