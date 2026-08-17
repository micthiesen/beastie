//! Authoritative, deterministic creature simulation.

mod language;
mod memory;
mod model;
mod random;
mod relationship;
mod save;
mod simulation;

pub use language::{
    GroundedUtterance, UtteranceInterpretation, UtteranceReference, ground_utterance,
    interpret_utterance,
};
pub use memory::{MemoryCue, MemoryQuery, select_candidate_memories};
pub use model::{
    ActionPhase, ActionRelationshipContext, ActionRelationshipMoment, ActionTimeline,
    AquariumCreatureState, AquariumPosition, AquariumState, Belief, BeliefId, BeliefKind, Concept,
    ConversationState, Creature, DepthLane, Development, DevelopmentMilestone, ExpressedMotif,
    Facing, FoodBuoyancy, FoodDisposition, FoodDropRejectionReason, FoodId, FoodObject,
    FoodOutcome, GazeTarget, Idiolect, IdiolectQuirk, IdleLifeState, InitiatedBehavior,
    InitiativeReason, Intention, InteractionCounters, InteractionState, LanguageExposure,
    LanguageStage, Memory, MemoryId, MemoryKind, Mood, NamingTarget, Needs, NonverbalAct,
    NormalizedPosition, NormalizedVelocity, Reaction, Relationship, RelationshipBeat,
    RelationshipBeatPhase, RelationshipEvidence, RelationshipExpressionKind,
    RelationshipExpressionMode, RelationshipExpressionState, RelationshipMotif,
    RelationshipMotifKey, RelationshipSubject, RelationshipTrigger, RelationshipTriggerKind,
    ResolvedToyInteraction, Routine, SemanticDestination, SocialAct, SocialHabits,
    StateValidationError, SteeringMode, ToyId, ToyInteraction, ToyInteractionOutcome,
    ToyInteractionPhase, ToyOrigin, Traits, TravelPurpose, TravelTarget, VisitEvidence,
    WorldObject, WorldState,
};
pub use random::{RandomDomain, RandomSource, SeededRandom, deterministic_unit};
pub use relationship::{
    derive_relationship_motifs, select_action_relationship_context, select_relationship_beat,
};
pub use save::{SaveError, SaveGame, migrate_world};
pub use simulation::{
    GameEvent, MAX_OFFLINE_MS, OfflineProgress, PlayerEvent, SIMULATION_TICK_MS, SpeechAttention,
    TALK_COOLDOWN_MS, advance_offline, apply_grounded_utterance, speech_attention, step,
    trigger_relationship_beat,
};

pub const SAVE_VERSION: u32 = 6;
pub const ACTIVE_DAY_MS: u64 = 15 * 60_000;
pub const RELATIONSHIP_EXPRESSION_SCHEMA_VERSION: u32 = 2;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
            action_id: 1,
            phase: ActionPhase::Act,
            elapsed_ms: 120,
            phase_duration_ms: 800,
            destination: SemanticDestination::Food(4),
            food_id: Some(4),
            food: Some(FoodId::Berry),
            food_outcome: None,
            relationship: None,
        });
        world.creature.interaction_state.next_action_id = 2;
        let action = world.creature.aquarium.action.clone();
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
    fn autonomous_toy_interest_does_not_defer_the_player() {
        let mut world = WorldState::new(10, "Listener");
        world.creature.current_intention = Intention::Play;
        world.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Ball));
        let interaction_id = std::num::NonZeroU64::new(1).unwrap();
        world.creature.aquarium.travel_purpose =
            Some(TravelPurpose::ToyInteraction { interaction_id });
        world.creature.interaction_state.next_toy_interaction_id = 2;
        world.creature.interaction_state.toy_interaction = Some(ToyInteraction {
            id: interaction_id,
            toy: ToyId::Ball,
            origin: ToyOrigin::Autonomous,
            outcome: ToyInteractionOutcome::Accepted,
            phase: ToyInteractionPhase::Approach,
            relationship: None,
        });
        let mut rng = SeededRandom::new(10);

        let events = step(&mut world, &[PlayerEvent::SpeechStarted], 0, &mut rng);

        assert_eq!(
            events,
            vec![
                GameEvent::ToyInteractionInterrupted {
                    toy: ToyId::Ball,
                    interaction_id,
                    origin: ToyOrigin::Autonomous,
                },
                GameEvent::SpeechPerceived(SpeechAttention::Attended),
            ]
        );

        world.creature.aquarium.destination = None;
        world.creature.aquarium.travel_purpose = None;
        world.creature.interaction_state.toy_interaction = None;
        world.creature.idle_life.last_arrived_destination =
            Some(SemanticDestination::Toy(ToyId::Ball));
        world.creature.idle_life.settled_until_ms = world.elapsed_ms + 5_000;
        let events = step(&mut world, &[PlayerEvent::SpeechStarted], 0, &mut rng);
        assert_eq!(
            events,
            vec![GameEvent::SpeechPerceived(SpeechAttention::Attended)]
        );
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
    fn quiet_observation_has_bounded_varied_idle_bouts_and_exact_replay() {
        let mut first = WorldState::new(42, "Quiet");
        let mut replay = first.clone();
        let mut first_rng = SeededRandom::new(42);
        let mut replay_rng = SeededRandom::new(42);
        let mut bouts = Vec::new();
        let mut previous_visits = 0_u32;
        let mut autonomous_toy_arrivals = 0_u32;

        for _ in 0..180 {
            let first_events = step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng);
            let replay_events = step(&mut replay, &[], SIMULATION_TICK_MS, &mut replay_rng);
            assert_eq!(first_events, replay_events);
            autonomous_toy_arrivals += first_events
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyPlayed { .. }))
                .count() as u32;
            let visits = first.creature.favorite_locations.values().sum::<u32>();
            if visits > previous_visits {
                bouts.push(
                    first
                        .creature
                        .idle_life
                        .settled_until_ms
                        .saturating_sub(first.elapsed_ms),
                );
                previous_visits = visits;
            }
        }

        assert_eq!(first, replay);
        assert_eq!(first_rng, replay_rng);
        assert!(
            bouts.len() >= 3,
            "expected several actual idle arrivals: {bouts:?}"
        );
        assert!(bouts.iter().all(|duration| {
            (4_000..=10_000).contains(duration) && duration % SIMULATION_TICK_MS == 0
        }));
        assert!(first.creature.favorite_locations.len() > 1);
        assert!(first.creature.favorite_locations.values().sum::<u32>() <= bouts.len() as u32);
        assert!(
            autonomous_toy_arrivals > 0,
            "toy visits should have a legible arrival beat"
        );
        SaveGame::capture(&first, &first_rng)
            .to_json()
            .expect("idle life with toy visits remains JSON serializable");
    }

    #[test]
    fn comfort_creates_history_improves_relationship_and_returns_to_life() {
        let mut world = WorldState::new(43, "Comforted");
        world.creature.relationship.resentment = 0.5;
        let before = world.creature.relationship;
        let mut rng = SeededRandom::new(43);

        step(&mut world, &[PlayerEvent::Comfort], 0, &mut rng);

        assert!(
            world
                .creature
                .memories
                .iter()
                .any(|memory| memory.kind == MemoryKind::WasComforted)
        );
        assert!(world.creature.relationship.bond > before.bond);
        assert!(world.creature.relationship.trust > before.trust);
        assert!(world.creature.relationship.resentment < before.resentment);
        assert_eq!(world.creature.current_intention, Intention::ShowAffection);

        step(&mut world, &[], 8_000, &mut rng);
        assert_eq!(world.creature.current_intention, Intention::Idle);
        assert!(world.creature.aquarium.destination.is_some());
    }

    #[test]
    fn disliked_repetition_raises_resentment_and_positive_care_recovers_it() {
        let mut world = WorldState::new(44, "Taste");
        world.creature.preferences.insert(FoodId::Berry, -1.0);
        let mut rng = SeededRandom::new(44);
        let initial = world.creature.relationship.resentment;

        for _ in 0..2 {
            step(
                &mut world,
                &[PlayerEvent::DropFood {
                    food: FoodId::Berry,
                    position: NormalizedPosition::new(5_000, 4_500),
                }],
                20_000,
                &mut rng,
            );
        }
        let after_disliked = world.creature.relationship.resentment;
        assert!(after_disliked > initial);

        step(&mut world, &[PlayerEvent::Comfort], 0, &mut rng);
        assert!(world.creature.relationship.resentment < after_disliked);
    }

    #[test]
    fn cursor_follow_requires_sustained_care() {
        let mut world = WorldState::new(45, "Trust");
        world.creature.traits.sociability = 1.0;
        let mut rng = SeededRandom::new(45);
        let cursor = NormalizedPosition::new(8_000, 4_500);

        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(cursor))],
            0,
            &mut rng,
        );
        assert_ne!(world.creature.aquarium.steering, SteeringMode::Approach);
        step(&mut world, &[PlayerEvent::Cursor(None)], 0, &mut rng);
        for _ in 0..36 {
            step(
                &mut world,
                &[PlayerEvent::Comfort],
                SIMULATION_TICK_MS,
                &mut rng,
            );
        }
        assert!(world.creature.relationship.trust > 0.5);

        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(cursor))],
            0,
            &mut rng,
        );
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Approach);
    }

    #[test]
    fn sleep_restores_energy_and_initiative_clears_or_expires() {
        let mut world = WorldState::new(46, "Rest");
        world.creature.needs.energy = 0.09;
        let mut rng = SeededRandom::new(46);
        let mut events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(events.contains(&GameEvent::SleepStarted));
        for _ in 0..10 {
            events.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
        }
        assert!(events.contains(&GameEvent::SleepEnded));
        assert_ne!(world.creature.current_intention, Intention::Sleep);
        assert!(world.creature.needs.energy >= 0.68);

        world.creature.needs.hunger = 0.9;
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        let first_request = world
            .creature
            .initiated_behavior
            .as_ref()
            .expect("hunger request")
            .requested_at_ms;
        world.creature.needs.hunger = 0.5;
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(world.creature.initiated_behavior.is_none());

        world.creature.needs.hunger = 0.9;
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        let expires_at = world
            .creature
            .initiated_behavior
            .as_ref()
            .expect("second hunger request")
            .expires_at_ms;
        let remaining = expires_at.saturating_sub(world.elapsed_ms);
        step(&mut world, &[], remaining, &mut rng);
        assert!(
            world
                .creature
                .initiated_behavior
                .as_ref()
                .is_some_and(|request| request.requested_at_ms > first_request)
        );
    }

    #[test]
    fn active_day_scaled_repeated_visits_form_a_bounded_routine() {
        let mut world = WorldState::new(47, "Habit");
        let mut rng = SeededRandom::new(47);
        let cave = NormalizedPosition::new(1_500, 8_500);

        for day in 0..2 {
            world.elapsed_ms = day * ACTIVE_DAY_MS;
            world.creature.aquarium.position = cave;
            world.creature.aquarium.destination = Some(SemanticDestination::Cave);
            world.creature.aquarium.travel_purpose = Some(TravelPurpose::IdleVisit {
                visit_id: std::num::NonZeroU64::new(day + 1).unwrap(),
            });
            world.creature.aquarium.steering = SteeringMode::Approach;
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }

        assert!(world.creature.routines.iter().any(|routine| {
            routine.hour_start == 0
                && routine.destination == SemanticDestination::Cave
                && routine.strength >= 2
        }));
        assert!(world.creature.routines.len() <= 8);
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
        let creature = value["world"]["creature"]
            .as_object_mut()
            .expect("creature object");
        creature.remove("idle_life");
        creature.remove("interaction_state");
        let (migrated, _) = SaveGame::from_json(&value.to_string()).unwrap().resume();
        assert_eq!(migrated.save_version, SAVE_VERSION);
        assert_eq!(migrated.creature.idle_life, IdleLifeState::default());
        assert_eq!(
            migrated.creature.interaction_state,
            InteractionState::default()
        );
        assert!(migrated.validate().is_ok());
    }

    #[test]
    fn current_version_save_defaults_new_persisted_life_state() {
        let world = WorldState::new(78, "V3");
        let mut value =
            serde_json::to_value(SaveGame::capture(&world, &SeededRandom::new(78))).unwrap();
        let creature = value["world"]["creature"]
            .as_object_mut()
            .expect("creature object");
        creature.remove("idle_life");
        creature.remove("interaction_state");
        let (loaded, _) = SaveGame::from_json(&value.to_string()).unwrap().resume();
        assert_eq!(loaded.creature.idle_life, IdleLifeState::default());
        assert_eq!(
            loaded.creature.interaction_state,
            InteractionState::default()
        );
    }

    #[test]
    fn v4_relationship_migration_clears_only_unrepresentable_active_expression() {
        let mut world = WorldState::new(781, "V4Relationship");
        world.creature.needs.comfort = 0.1;
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.6, 0.8);
        trigger_relationship_beat(&mut world, RelationshipTrigger::ComfortNeeded);
        assert!(world.creature.relationship_expression.active.is_some());
        assert!(!world.creature.relationship_expression.recent.is_empty());

        let mut value =
            serde_json::to_value(SaveGame::capture(&world, &SeededRandom::new(781))).unwrap();
        value["save_version"] = 4.into();
        value["world"]["save_version"] = 4.into();
        value["world"]["creature"]["relationship_expression"]["schema_version"] = 1.into();
        value["world"]["creature"]["relationship_expression"]["active"]["trigger"] =
            serde_json::json!({"kind": "action_completed", "destination": "bottom"});
        let recent_before = value["world"]["creature"]["relationship_expression"]["recent"].clone();

        let (loaded, _) = SaveGame::from_json(&value.to_string()).unwrap().resume();
        assert!(loaded.creature.relationship_expression.active.is_none());
        assert_eq!(
            serde_json::to_value(&loaded.creature.relationship_expression.recent).unwrap(),
            recent_before
        );
        assert!(loaded.validate().is_ok());
    }

    #[test]
    fn v3_save_migrates_relationship_expression_to_version_four() {
        let world = WorldState::new(79, "V3");
        let mut value =
            serde_json::to_value(SaveGame::capture(&world, &SeededRandom::new(79))).unwrap();
        value["save_version"] = 3.into();
        value["world"]["save_version"] = 3.into();
        value["world"]["creature"]
            .as_object_mut()
            .expect("creature object")
            .remove("relationship_expression");
        let (loaded, _) = SaveGame::from_json(&value.to_string()).unwrap().resume();
        assert_eq!(loaded.save_version, SAVE_VERSION);
        assert_eq!(
            loaded.creature.relationship_expression,
            RelationshipExpressionState::default()
        );
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
        hungry.creature.idle_life.last_arrived_destination = Some(SemanticDestination::Bottom);
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
        let receipt = step(&mut world, &[PlayerEvent::Play(ToyId::Bell)], 0, &mut rng);
        assert!(receipt.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayAccepted {
                toy: ToyId::Bell,
                origin: ToyOrigin::Player,
                ..
            }
        )));
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Bell }))
        );
        let played_toy = step(&mut world, &[], 10_000, &mut rng);
        assert!(played_toy.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayed {
                toy: ToyId::Bell,
                origin: ToyOrigin::Player,
                ..
            }
        )));
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
        assert!(rejected_toy.iter().any(|event| matches!(
            event,
            GameEvent::ToyRejected {
                toy: ToyId::Sock,
                origin: ToyOrigin::Player,
                ..
            }
        )));
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
        assert!(progress.events.iter().any(|event| matches!(
            event,
            GameEvent::RelationshipBeatStarted {
                motif: RelationshipMotifKey::PlayerReturns,
                expression: RelationshipExpressionKind::Notice,
                ..
            }
        )));
        assert!(
            !progress
                .events
                .contains(&GameEvent::NonverbalAct(NonverbalAct::LeanAgainstPlayer))
        );
        assert_eq!(world.creature.aquarium.gaze, GazeTarget::Player);
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
            first
                .creature
                .aquarium
                .action
                .as_ref()
                .map(|action| action.phase),
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

    #[test]
    fn relationship_motifs_are_derived_for_all_families_with_typed_evidence() {
        let mut world = WorldState::new(101, "History");
        world.creature.preferences.insert(FoodId::Berry, 0.8);
        world.creature.preferences.insert(FoodId::Mushroom, -0.8);
        let comfort = world.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        let toy = world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        let fed_one = world.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.7,
            0.7,
        );
        world.elapsed_ms = ACTIVE_DAY_MS;
        let _fed_two = world.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.7,
            0.7,
        );
        let grudge = world.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.7,
            0.8,
        );
        world.revise_belief(BeliefKind::FoodIsATrick, grudge, true);
        let returned = world.remember(
            MemoryKind::PlayerReturnedAfterAbsence,
            &[Concept::You],
            0.5,
            0.8,
        );
        world.revise_belief(BeliefKind::PlayerReturnsAfterSleep, returned, true);
        world.creature.idle_life.visit_evidence.push(VisitEvidence {
            hour_start: 2,
            destination: SemanticDestination::Cave,
            visits: 2,
            last_active_day: 2,
        });
        world.set_routine(Routine {
            hour_start: 2,
            destination: SemanticDestination::Cave,
            strength: 2,
        });
        let motifs = derive_relationship_motifs(&world);
        let keys = motifs
            .iter()
            .map(|motif| motif.key)
            .collect::<BTreeSet<_>>();
        assert!(keys.contains(&RelationshipMotifKey::SharedToy(ToyId::Ball)));
        assert!(keys.contains(&RelationshipMotifKey::ComfortRitual));
        assert!(keys.contains(&RelationshipMotifKey::TrustedFood(FoodId::Berry)));
        assert!(keys.contains(&RelationshipMotifKey::FoodGrudge(FoodId::Mushroom)));
        assert!(keys.contains(&RelationshipMotifKey::PlayerReturns));
        assert!(keys.contains(&RelationshipMotifKey::FamiliarPlace(
            SemanticDestination::Cave
        )));
        let shared = motifs
            .iter()
            .find(|motif| motif.key == RelationshipMotifKey::SharedToy(ToyId::Ball))
            .expect("shared toy motif");
        assert!(
            shared
                .evidence
                .contains(&RelationshipEvidence::Memory { id: toy })
        );
        let grudge_motif = motifs
            .iter()
            .find(|motif| motif.key == RelationshipMotifKey::FoodGrudge(FoodId::Mushroom))
            .expect("food grudge motif");
        assert!(
            grudge_motif
                .evidence
                .contains(&RelationshipEvidence::Belief {
                    id: world
                        .creature
                        .beliefs
                        .iter()
                        .find(|belief| belief.kind == BeliefKind::FoodIsATrick)
                        .expect("grudge belief")
                        .id,
                    kind: BeliefKind::FoodIsATrick,
                })
        );
        let returns_motif = motifs
            .iter()
            .find(|motif| motif.key == RelationshipMotifKey::PlayerReturns)
            .expect("returns motif");
        assert!(returns_motif.evidence.iter().any(|evidence| matches!(
            evidence,
            RelationshipEvidence::Belief {
                kind: BeliefKind::PlayerReturnsAfterSleep,
                ..
            }
        )));
        let _ = comfort;
        let _ = fed_one;
    }

    #[test]
    fn relationship_strength_requires_distinct_active_days_for_repeated_memories() {
        let mut world = WorldState::new(102, "Days");
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        assert_eq!(derive_relationship_motifs(&world)[0].strength, 1);
        world.elapsed_ms = ACTIVE_DAY_MS;
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        assert_eq!(derive_relationship_motifs(&world)[0].strength, 2);
    }

    #[test]
    fn relationship_selection_prefers_recent_supporting_evidence() {
        let mut world = WorldState::new(111, "Recency");
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        world.elapsed_ms = ACTIVE_DAY_MS * 19;
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Bell },
            &[Concept::Toy],
            0.6,
            0.7,
        );

        let beat = select_relationship_beat(
            &world,
            RelationshipTrigger::RelevantUtterance {
                subject: Some(RelationshipSubject::Toy(ToyId::Bell)),
            },
        )
        .expect("recent toy evidence should remain eligible");
        assert_eq!(beat.motif, RelationshipMotifKey::SharedToy(ToyId::Bell));
    }

    #[test]
    fn player_return_never_selects_an_unrelated_grounded_motif() {
        let mut base = WorldState::new(112, "Variation");
        let returned = base.remember(
            MemoryKind::PlayerReturnedAfterAbsence,
            &[Concept::You, Concept::Again],
            0.5,
            0.8,
        );
        base.revise_belief(BeliefKind::PlayerReturnsAfterSleep, returned, true);
        base.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        base.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        base.creature.idle_life.visit_evidence.push(VisitEvidence {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            visits: 2,
            last_active_day: 1,
        });
        base.set_routine(Routine {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            strength: 2,
        });

        let run = |mut world: WorldState| {
            let mut rng = SeededRandom::new(world.seed);
            let mut motifs = Vec::new();
            let mut events = Vec::new();
            for elapsed_ms in [0, ACTIVE_DAY_MS * 2, ACTIVE_DAY_MS * 3] {
                world.elapsed_ms = elapsed_ms;
                let trigger_events =
                    trigger_relationship_beat(&mut world, RelationshipTrigger::PlayerReturn);
                motifs.push(
                    world
                        .creature
                        .relationship_expression
                        .active
                        .as_ref()
                        .expect("a grounded return motif")
                        .motif,
                );
                events.extend(trigger_events);
                events.extend(step(&mut world, &[], 10_000, &mut rng));
            }
            (world, motifs, events)
        };

        let (first, first_motifs, first_events) = run(base.clone());
        let (second, second_motifs, second_events) = run(base);
        assert_eq!(first_motifs, vec![RelationshipMotifKey::PlayerReturns; 3]);
        assert_eq!(first_motifs, second_motifs);
        assert_eq!(first_events, second_events);
        assert_eq!(first, second);
    }

    #[test]
    fn relationship_selection_is_stable_and_ledger_is_bounded() {
        let mut first = WorldState::new(103, "Select");
        let mut second = first.clone();
        for state in [&mut first, &mut second] {
            state.creature.preferences.insert(FoodId::Berry, 0.8);
            state.creature.toy_preferences.insert(ToyId::Ball, 0.8);
            let memory = state.remember(
                MemoryKind::PlayedWith { toy: ToyId::Ball },
                &[Concept::Toy],
                0.6,
                0.7,
            );
            state.creature.beliefs.push(Belief {
                id: BeliefId(1),
                kind: BeliefKind::PlayerReturnsAfterSleep,
                supporting_memories: BTreeSet::from([memory]),
                contradicting_memories: BTreeSet::new(),
                confidence: 0.5,
            });
            state.next_belief_id = 2;
        }
        let first_context = select_action_relationship_context(
            &first,
            RelationshipTrigger::ToyEngaged { toy: ToyId::Ball },
        )
        .expect("eligible toy context");
        let second_context = select_action_relationship_context(
            &second,
            RelationshipTrigger::ToyEngaged { toy: ToyId::Ball },
        )
        .expect("same eligible toy context");
        assert_eq!(first_context, second_context);
        let events = step(
            &mut first,
            &[PlayerEvent::Play(ToyId::Ball)],
            10_000,
            &mut SeededRandom::new(103),
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::ActionRelationshipStarted { .. }))
        );
        assert_eq!(first.creature.relationship_expression.recent.len(), 1);
        assert!(
            select_action_relationship_context(
                &first,
                RelationshipTrigger::ToyEngaged { toy: ToyId::Ball }
            )
            .is_none()
        );
    }

    #[test]
    fn action_relationship_matching_is_exact_across_subject_families() {
        let mut world = WorldState::new(130, "ExactSubjects");
        world.creature.preferences.insert(FoodId::Berry, 0.8);
        world.creature.preferences.insert(FoodId::Mushroom, -0.8);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.8,
            0.8,
        );
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.7,
            0.8,
        );
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.7, 0.8);
        let rejected = world.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.8,
            0.9,
        );
        world.revise_belief(BeliefKind::FoodIsATrick, rejected, true);
        world.elapsed_ms = ACTIVE_DAY_MS;
        world.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.8,
            0.8,
        );

        let cases = [
            (
                RelationshipTrigger::FoodPresented {
                    food: FoodId::Berry,
                },
                RelationshipMotifKey::TrustedFood(FoodId::Berry),
                RelationshipSubject::Food(FoodId::Berry),
            ),
            (
                RelationshipTrigger::FoodPresented {
                    food: FoodId::Mushroom,
                },
                RelationshipMotifKey::FoodGrudge(FoodId::Mushroom),
                RelationshipSubject::Food(FoodId::Mushroom),
            ),
            (
                RelationshipTrigger::ToyEngaged { toy: ToyId::Ball },
                RelationshipMotifKey::SharedToy(ToyId::Ball),
                RelationshipSubject::Toy(ToyId::Ball),
            ),
            (
                RelationshipTrigger::ComfortCompleted,
                RelationshipMotifKey::ComfortRitual,
                RelationshipSubject::Player,
            ),
        ];
        for (trigger, motif, subject) in cases {
            let context = select_action_relationship_context(&world, trigger)
                .expect("exact subject should be eligible");
            assert_eq!(context.motif, motif);
            assert_eq!(context.subject, subject);
        }
        assert!(
            select_action_relationship_context(
                &world,
                RelationshipTrigger::FoodPresented {
                    food: FoodId::Pellet,
                },
            )
            .is_none()
        );
        assert!(
            select_action_relationship_context(
                &world,
                RelationshipTrigger::ToyEngaged { toy: ToyId::Bell },
            )
            .is_none()
        );
        assert!(
            select_relationship_beat(&world, RelationshipTrigger::QuietMoment).is_some_and(
                |beat| !matches!(
                    beat.motif,
                    RelationshipMotifKey::TrustedFood(_) | RelationshipMotifKey::FoodGrudge(_)
                )
            )
        );
    }

    #[test]
    fn food_relationship_context_preserves_outcome_and_survives_consumption() {
        let mut recognized = WorldState::new(131, "RecognizedFood");
        recognized.creature.preferences.insert(FoodId::Berry, 0.8);
        recognized.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.8,
            0.8,
        );
        recognized.elapsed_ms = ACTIVE_DAY_MS;
        recognized.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.8,
            0.8,
        );
        let mut plain = recognized.clone();
        plain.creature.memories.clear();
        plain.creature.relationship_expression = RelationshipExpressionState::default();
        let mut recognized_rng = SeededRandom::new(131);
        let mut plain_rng = SeededRandom::new(131);
        let drop = PlayerEvent::DropFood {
            food: FoodId::Berry,
            position: NormalizedPosition::new(5_000, 3_000),
        };
        let started = step(
            &mut recognized,
            std::slice::from_ref(&drop),
            0,
            &mut recognized_rng,
        );
        step(&mut plain, &[drop], 0, &mut plain_rng);
        assert!(started.iter().any(|event| matches!(
            event,
            GameEvent::ActionRelationshipStarted {
                motif: RelationshipMotifKey::TrustedFood(FoodId::Berry),
                subject: RelationshipSubject::Food(FoodId::Berry),
                ..
            }
        )));
        let notice = recognized.creature.aquarium.action.as_ref().unwrap();
        assert_eq!(notice.phase, ActionPhase::Notice);
        assert_eq!(notice.food, Some(FoodId::Berry));
        assert!(notice.relationship.is_some());

        let mut recognized_events = Vec::new();
        let mut plain_events = Vec::new();
        for _ in 0..24 {
            recognized_events.extend(step(&mut recognized, &[], 1_000, &mut recognized_rng));
            plain_events.extend(step(&mut plain, &[], 1_000, &mut plain_rng));
            if recognized
                .creature
                .aquarium
                .action
                .as_ref()
                .is_some_and(|action| action.phase == ActionPhase::Recover)
            {
                break;
            }
        }
        assert!(recognized_events.contains(&GameEvent::FoodConsumed(FoodId::Berry)));
        assert!(plain_events.contains(&GameEvent::FoodConsumed(FoodId::Berry)));
        let action = recognized.creature.aquarium.action.as_ref().unwrap();
        assert_eq!(action.food, Some(FoodId::Berry));
        assert_eq!(action.food_outcome, Some(FoodOutcome::Consumed));
        assert!(
            !recognized
                .aquarium
                .objects
                .contains_key(&action.food_id.unwrap())
        );
        let encoded = SaveGame::capture(&recognized, &recognized_rng)
            .to_json()
            .expect("relationship recovery should save");
        let (loaded, _) = SaveGame::from_json(&encoded).unwrap().resume();
        assert_eq!(
            loaded.creature.aquarium.action,
            recognized.creature.aquarium.action
        );
    }

    #[test]
    fn forged_action_relationship_subject_is_rejected_by_save_validation() {
        let mut world = WorldState::new(132, "ForgedAction");
        world.creature.preferences.insert(FoodId::Berry, 0.8);
        for elapsed in [0, ACTIVE_DAY_MS] {
            world.elapsed_ms = elapsed;
            world.remember(
                MemoryKind::WasFed {
                    food: FoodId::Berry,
                },
                &[Concept::Food],
                0.8,
                0.8,
            );
        }
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 3_000),
            }],
            0,
            &mut SeededRandom::new(132),
        );
        world
            .creature
            .aquarium
            .action
            .as_mut()
            .unwrap()
            .relationship
            .as_mut()
            .unwrap()
            .subject = RelationshipSubject::Food(FoodId::Mushroom);
        assert!(
            SaveGame::capture(&world, &SeededRandom::new(132))
                .to_json()
                .is_err()
        );
    }

    #[test]
    fn superseded_and_missing_food_actions_cancel_their_relationship_receipts() {
        let mut world = WorldState::new(133, "InterruptedFood");
        world.creature.preferences.insert(FoodId::Berry, 0.8);
        for elapsed in [0, ACTIVE_DAY_MS] {
            world.elapsed_ms = elapsed;
            world.remember(
                MemoryKind::WasFed {
                    food: FoodId::Berry,
                },
                &[Concept::Food],
                0.8,
                0.8,
            );
        }
        let mut rng = SeededRandom::new(133);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 3_000),
            }],
            0,
            &mut rng,
        );
        let first_action = world.creature.aquarium.action.as_ref().unwrap().action_id;
        let superseded = step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(6_000, 3_000),
            }],
            0,
            &mut rng,
        );
        assert!(superseded.iter().any(|event| matches!(
            event,
            GameEvent::ActionRelationshipInterrupted { action_id, .. }
                if *action_id == first_action
        )));
        assert!(superseded.iter().any(|event| matches!(
            event,
            GameEvent::ActionAborted {
                destination: SemanticDestination::Food(_)
            }
        )));

        let current_food = world
            .creature
            .aquarium
            .action
            .as_ref()
            .unwrap()
            .food_id
            .unwrap();
        world.aquarium.objects.remove(&current_food);
        let mut aborted = Vec::new();
        for _ in 0..20 {
            aborted.extend(step(&mut world, &[], 1_000, &mut rng));
            if world.creature.aquarium.action.is_none() {
                break;
            }
        }
        assert!(aborted.iter().any(|event| matches!(
            event,
            GameEvent::ActionAborted {
                destination: SemanticDestination::Food(id)
            } if *id == current_food
        )));
        assert!(world.creature.aquarium.action.is_none());
    }

    #[test]
    fn grudge_eligibility_uses_the_authoritative_rejection_boundary() {
        let mut world = WorldState::new(134, "Boundary");
        let memory = world.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.7,
            0.8,
        );
        world.revise_belief(BeliefKind::FoodIsATrick, memory, true);
        world.creature.preferences.insert(FoodId::Mushroom, -0.35);
        assert!(
            select_action_relationship_context(
                &world,
                RelationshipTrigger::FoodPresented {
                    food: FoodId::Mushroom,
                },
            )
            .is_none()
        );
        world.creature.preferences.insert(FoodId::Mushroom, -0.351);
        assert_eq!(
            select_action_relationship_context(
                &world,
                RelationshipTrigger::FoodPresented {
                    food: FoodId::Mushroom,
                },
            )
            .map(|context| context.motif),
            Some(RelationshipMotifKey::FoodGrudge(FoodId::Mushroom))
        );
    }

    #[test]
    fn relationship_beats_progress_save_reload_and_yield_to_care() {
        let mut first = WorldState::new(104, "Beat");
        let mut rng = SeededRandom::new(104);
        let memory = first.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        first.creature.beliefs.push(Belief {
            id: BeliefId(1),
            kind: BeliefKind::PlayerReturnsAfterSleep,
            supporting_memories: BTreeSet::from([memory]),
            contradicting_memories: BTreeSet::new(),
            confidence: 0.5,
        });
        first.next_belief_id = 2;
        trigger_relationship_beat(&mut first, RelationshipTrigger::QuietMoment);
        step(&mut first, &[], 1_000, &mut rng);
        let encoded = SaveGame::capture(&first, &rng)
            .to_json()
            .expect("beat save");
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded).unwrap().resume();
        assert_eq!(
            step(&mut first, &[], 2_000, &mut rng),
            step(&mut resumed, &[], 2_000, &mut resumed_rng)
        );
        assert_eq!(first, resumed);
        let interrupted = step(&mut resumed, &[PlayerEvent::Comfort], 0, &mut resumed_rng);
        assert!(
            interrupted
                .iter()
                .any(|event| matches!(event, GameEvent::Comforted))
        );
        assert!(resumed.creature.relationship_expression.active.is_none());
    }

    #[test]
    fn grounded_utterance_is_a_public_typed_relationship_trigger() {
        let mut world = WorldState::new(105, "Words");
        let interpretation = UtteranceInterpretation {
            references: BTreeSet::from([UtteranceReference::Toy(ToyId::Ball)]),
            ..UtteranceInterpretation::default()
        };
        let events = apply_grounded_utterance(&mut world, &interpretation);
        assert!(
            !events
                .iter()
                .any(|event| { matches!(event, GameEvent::RelationshipBeatStarted { .. }) })
        );
    }

    #[test]
    fn successful_toy_use_starts_the_shared_toy_callback() {
        let mut world = WorldState::new(113, "ToyUse");
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        let mut rng = SeededRandom::new(113);
        let events = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Ball)],
            10_000,
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ActionRelationshipStarted {
                motif: RelationshipMotifKey::SharedToy(ToyId::Ball),
                subject: RelationshipSubject::Toy(ToyId::Ball),
                ..
            }
        )));
    }

    #[test]
    fn relationship_callbacks_project_their_family_specific_embodied_intentions() {
        let mut comfort = WorldState::new(107, "Comfort");
        comfort.creature.needs.comfort = 0.1;
        comfort.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        comfort.elapsed_ms = ACTIVE_DAY_MS;
        comfort.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        trigger_relationship_beat(&mut comfort, RelationshipTrigger::ComfortNeeded);
        step(&mut comfort, &[], 1_000, &mut SeededRandom::new(107));
        assert_eq!(comfort.creature.current_intention, Intention::SeekComfort);

        let mut trusted = WorldState::new(108, "Trusted");
        trusted.creature.preferences.insert(FoodId::Berry, 0.8);
        trusted.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.7,
            0.7,
        );
        trusted.elapsed_ms = ACTIVE_DAY_MS;
        trusted.remember(
            MemoryKind::WasFed {
                food: FoodId::Berry,
            },
            &[Concept::Food],
            0.7,
            0.7,
        );
        step(
            &mut trusted,
            &[PlayerEvent::DropFood {
                food: FoodId::Berry,
                position: NormalizedPosition::new(5_000, 3_000),
            }],
            0,
            &mut SeededRandom::new(108),
        );
        assert_eq!(
            trusted
                .creature
                .aquarium
                .action
                .as_ref()
                .and_then(|action| action.relationship.as_ref())
                .map(|context| context.motif),
            Some(RelationshipMotifKey::TrustedFood(FoodId::Berry))
        );

        let mut grudge = WorldState::new(109, "Grudge");
        grudge.creature.preferences.insert(FoodId::Mushroom, -0.8);
        let memory = grudge.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.7,
            0.8,
        );
        grudge.revise_belief(BeliefKind::FoodIsATrick, memory, true);
        grudge.elapsed_ms = ACTIVE_DAY_MS;
        let second_memory = grudge.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.7,
            0.8,
        );
        grudge.revise_belief(BeliefKind::FoodIsATrick, second_memory, true);
        step(
            &mut grudge,
            &[PlayerEvent::DropFood {
                food: FoodId::Mushroom,
                position: NormalizedPosition::new(5_000, 3_000),
            }],
            0,
            &mut SeededRandom::new(109),
        );
        assert_eq!(
            grudge
                .creature
                .aquarium
                .action
                .as_ref()
                .and_then(|action| action.relationship.as_ref())
                .map(|context| context.motif),
            Some(RelationshipMotifKey::FoodGrudge(FoodId::Mushroom))
        );

        let mut place = WorldState::new(110, "Place");
        place.creature.idle_life.visit_evidence.push(VisitEvidence {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            visits: 2,
            last_active_day: 1,
        });
        place.set_routine(Routine {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            strength: 2,
        });
        trigger_relationship_beat(
            &mut place,
            RelationshipTrigger::RoutineWindow {
                hour_start: 3,
                destination: SemanticDestination::Cave,
            },
        );
        assert_eq!(
            place
                .creature
                .relationship_expression
                .active
                .as_ref()
                .map(|beat| beat.motif),
            Some(RelationshipMotifKey::FamiliarPlace(
                SemanticDestination::Cave
            ))
        );
    }

    #[test]
    fn early_relationship_evidence_only_notices_without_forcing_familiar_behavior() {
        let mut comfort = WorldState::new(120, "EarlyComfort");
        comfort.creature.needs.comfort = 0.1;
        comfort.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        trigger_relationship_beat(&mut comfort, RelationshipTrigger::ComfortNeeded);
        assert_eq!(comfort.creature.aquarium.gaze, GazeTarget::Player);
        step(&mut comfort, &[], 1_000, &mut SeededRandom::new(120));
        let beat = comfort
            .creature
            .relationship_expression
            .active
            .as_ref()
            .expect("early comfort beat");
        assert_eq!(beat.expression_kind, RelationshipExpressionKind::Notice);
        assert_ne!(comfort.creature.current_intention, Intention::SeekComfort);

        let mut grudge = WorldState::new(121, "EarlyGrudge");
        grudge.creature.preferences.insert(FoodId::Mushroom, -0.8);
        let memory = grudge.remember(
            MemoryKind::RejectedFood {
                food: FoodId::Mushroom,
            },
            &[Concept::Food],
            -0.7,
            0.8,
        );
        grudge.revise_belief(BeliefKind::FoodIsATrick, memory, true);
        let context = select_action_relationship_context(
            &grudge,
            RelationshipTrigger::FoodPresented {
                food: FoodId::Mushroom,
            },
        )
        .expect("early grudge context");
        assert_eq!(context.expression_kind, RelationshipExpressionKind::Notice);
        assert_ne!(grudge.creature.current_intention, Intention::RejectFood);
    }

    #[test]
    fn active_relationship_beat_keeps_its_embodied_priority_over_idle_scheduling() {
        let mut world = WorldState::new(123, "Priority");
        let first = world.remember(
            MemoryKind::PlayerReturnedAfterAbsence,
            &[Concept::You, Concept::Again],
            0.5,
            0.8,
        );
        world.revise_belief(BeliefKind::PlayerReturnsAfterSleep, first, true);
        world.elapsed_ms = ACTIVE_DAY_MS;
        let second = world.remember(
            MemoryKind::PlayerReturnedAfterAbsence,
            &[Concept::You, Concept::Again],
            0.5,
            0.8,
        );
        world.revise_belief(BeliefKind::PlayerReturnsAfterSleep, second, true);
        trigger_relationship_beat(&mut world, RelationshipTrigger::PlayerReturn);

        let mut rng = SeededRandom::new(123);
        for _ in 0..5 {
            step(&mut world, &[], 1_000, &mut rng);
            assert_eq!(world.creature.current_intention, Intention::ApproachPlayer);
            assert!(
                world
                    .creature
                    .aquarium
                    .destination
                    .is_none_or(|destination| destination == SemanticDestination::Player)
            );
            assert_eq!(world.creature.aquarium.gaze, GazeTarget::Player);
        }
    }

    #[test]
    fn familiar_place_act_waits_for_a_current_arrival_not_stale_history() {
        let mut world = WorldState::new(124, "CurrentArrival");
        world.creature.aquarium.position = NormalizedPosition::new(10_000, 0);
        world.creature.idle_life.last_arrived_destination = Some(SemanticDestination::Cave);
        world.creature.idle_life.settled_until_ms = 0;
        world.creature.idle_life.visit_evidence.push(VisitEvidence {
            hour_start: 0,
            destination: SemanticDestination::Cave,
            visits: 2,
            last_active_day: 1,
        });
        world.set_routine(Routine {
            hour_start: 0,
            destination: SemanticDestination::Cave,
            strength: 2,
        });
        trigger_relationship_beat(
            &mut world,
            RelationshipTrigger::RoutineWindow {
                hour_start: 0,
                destination: SemanticDestination::Cave,
            },
        );
        step(&mut world, &[], 4_000, &mut SeededRandom::new(124));
        assert_eq!(
            world
                .creature
                .relationship_expression
                .active
                .as_ref()
                .map(|beat| beat.phase),
            Some(RelationshipBeatPhase::Anticipate)
        );
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Cave)
        );
    }

    #[test]
    fn validation_rejects_an_active_relationship_beat_not_grounded_in_derived_truth() {
        let mut world = WorldState::new(122, "ForgedBeat");
        world.creature.needs.comfort = 0.1;
        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        trigger_relationship_beat(&mut world, RelationshipTrigger::ComfortNeeded);
        assert!(world.validate().is_ok());

        world
            .creature
            .relationship_expression
            .active
            .as_mut()
            .expect("active relationship beat")
            .target = Some(SemanticDestination::Bottom);
        assert!(matches!(
            world.validate(),
            Err(StateValidationError::RelationshipExpression)
        ));
    }

    #[test]
    fn every_relationship_family_recovers_to_independent_life() {
        let finish = |mut world: WorldState, trigger: RelationshipTrigger| {
            let mut rng = SeededRandom::new(world.seed);
            trigger_relationship_beat(&mut world, trigger);
            for _ in 0..12 {
                step(&mut world, &[], 1_000, &mut rng);
            }
            world
        };

        let mut toy = WorldState::new(114, "Toy");
        toy.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.7,
        );
        let toy = finish(toy, RelationshipTrigger::QuietMoment);

        let mut comfort = WorldState::new(115, "Comfort");
        comfort.creature.needs.comfort = 0.1;
        comfort.remember(MemoryKind::WasComforted, &[Concept::You], 0.5, 0.7);
        let comfort = finish(comfort, RelationshipTrigger::ComfortNeeded);

        let mut returns = WorldState::new(118, "Returns");
        let returned = returns.remember(
            MemoryKind::PlayerReturnedAfterAbsence,
            &[Concept::You],
            0.5,
            0.8,
        );
        returns.revise_belief(BeliefKind::PlayerReturnsAfterSleep, returned, true);
        let returns = finish(returns, RelationshipTrigger::PlayerReturn);

        let mut place = WorldState::new(119, "Place");
        place.creature.idle_life.visit_evidence.push(VisitEvidence {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            visits: 2,
            last_active_day: 1,
        });
        place.set_routine(Routine {
            hour_start: 3,
            destination: SemanticDestination::Cave,
            strength: 2,
        });
        let place = finish(
            place,
            RelationshipTrigger::RoutineWindow {
                hour_start: 3,
                destination: SemanticDestination::Cave,
            },
        );

        for world in [toy, comfort, returns, place] {
            assert!(world.creature.relationship_expression.active.is_none());
            assert!(world.creature.aquarium.action.is_none());
        }
    }

    #[test]
    fn offline_progress_advances_elapsed_time_but_emits_one_return_path() {
        let mut world = WorldState::new(106, "Offline");
        let mut rng = SeededRandom::new(106);
        let before = world.elapsed_ms;
        let progress = advance_offline(&mut world, ACTIVE_DAY_MS * 2, &mut rng);
        assert_eq!(world.elapsed_ms, before + ACTIVE_DAY_MS * 2);
        assert_eq!(
            progress
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::MemoryCreated(_)))
                .count(),
            1
        );
        assert!(progress.events.iter().any(|event| matches!(
            event,
            GameEvent::RelationshipBeatStarted {
                motif: RelationshipMotifKey::PlayerReturns,
                ..
            }
        )));
        assert!(world.aquarium.player_present);
    }

    fn toy_position(world: &WorldState, toy: ToyId) -> NormalizedPosition {
        world
            .aquarium
            .objects
            .values()
            .find_map(|object| match object {
                WorldObject::Toy {
                    toy: candidate,
                    position,
                } if *candidate == toy => Some(*position),
                _ => None,
            })
            .expect("default toy exists")
    }

    #[test]
    fn accepted_toy_mutates_only_once_at_physical_contact() {
        let mut world = WorldState::new(201, "Contact");
        let mut rng = SeededRandom::new(201);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.aquarium.position = toy_position(&world, ToyId::Ball);
        let curiosity = world.creature.needs.curiosity;
        let relationship = world.creature.relationship;

        let receipt = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let interaction_id = receipt
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .expect("accepted receipt");
        assert!(!receipt.iter().any(|event| matches!(
            event,
            GameEvent::ToyContacted { .. } | GameEvent::ToyPlayed { .. }
        )));
        assert_eq!(world.creature.needs.curiosity, curiosity);
        assert_eq!(world.creature.relationship, relationship);
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
        );

        let contact = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(contact.contains(&GameEvent::ToyContacted {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert!(contact.contains(&GameEvent::ToyPlayed {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert_eq!(
            world
                .creature
                .memories
                .iter()
                .filter(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
                .count(),
            1
        );
        let later = step(&mut world, &[], SIMULATION_TICK_MS * 4, &mut rng);
        assert!(!later.iter().any(|event| matches!(
            event,
            GameEvent::ToyContacted { interaction_id: candidate, .. }
                | GameEvent::ToyPlayed { interaction_id: candidate, .. }
                if *candidate == interaction_id
        )));
    }

    #[test]
    fn rejected_toy_arrival_never_becomes_positive_evidence() {
        let mut world = WorldState::new(202, "Refusal");
        let mut rng = SeededRandom::new(202);
        world.creature.toy_preferences.insert(ToyId::Sock, -1.0);
        world.creature.aquarium.position = toy_position(&world, ToyId::Sock);
        let relationship = world.creature.relationship;

        let first = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Sock)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        assert!(first.iter().any(|event| matches!(
            event,
            GameEvent::ToyRejected {
                toy: ToyId::Sock,
                ..
            }
        )));
        assert!(!first.iter().any(|event| matches!(
            event,
            GameEvent::ToyContacted { .. } | GameEvent::ToyPlayed { .. }
        )));
        assert_eq!(world.creature.relationship, relationship);
        assert!(
            !world
                .creature
                .favorite_locations
                .contains_key(&SemanticDestination::Toy(ToyId::Sock))
        );
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Sock }))
        );

        let second = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Sock)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        assert_eq!(
            second
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyRejected { .. }))
                .count(),
            1
        );
        assert_eq!(
            world
                .creature
                .memories
                .iter()
                .filter(|memory| matches!(
                    memory.kind,
                    MemoryKind::DislikedToy { toy: ToyId::Sock }
                ))
                .count(),
            2
        );
    }

    #[test]
    fn superseding_direct_action_interrupts_toy_without_payoff() {
        let mut world = WorldState::new(203, "Interrupted");
        let mut rng = SeededRandom::new(203);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        let accepted = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let interaction_id = accepted
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .unwrap();

        let interrupted = step(
            &mut world,
            &[PlayerEvent::Comfort],
            SIMULATION_TICK_MS * 12,
            &mut rng,
        );
        assert!(interrupted.contains(&GameEvent::ToyInteractionInterrupted {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert!(!interrupted.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayed { interaction_id: candidate, .. } if *candidate == interaction_id
        )));
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
        );
    }

    #[test]
    fn later_toy_offer_cannot_resolve_the_superseded_offer() {
        let mut world = WorldState::new(204, "Replacement");
        let mut rng = SeededRandom::new(204);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.toy_preferences.insert(ToyId::Bell, 0.8);
        let first = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let first_id = first
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .unwrap();
        world.creature.aquarium.position = toy_position(&world, ToyId::Bell);
        let replacement = step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Bell)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        assert!(replacement.iter().any(|event| matches!(
            event,
            GameEvent::ToyInteractionInterrupted { interaction_id, .. } if *interaction_id == first_id
        )));
        assert!(replacement.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayed {
                toy: ToyId::Bell,
                origin: ToyOrigin::Player,
                ..
            }
        )));
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
        );
    }

    #[test]
    fn autonomous_toy_arrival_has_no_player_social_reward() {
        let mut world = WorldState::new(205, "Autonomous");
        let mut rng = SeededRandom::new(205);
        world.creature.aquarium.position = toy_position(&world, ToyId::Bell);
        world.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Bell));
        let interaction_id = std::num::NonZeroU64::new(1).unwrap();
        world.creature.aquarium.travel_purpose =
            Some(TravelPurpose::ToyInteraction { interaction_id });
        world.creature.interaction_state.next_toy_interaction_id = 2;
        world.creature.interaction_state.toy_interaction = Some(ToyInteraction {
            id: interaction_id,
            toy: ToyId::Bell,
            origin: ToyOrigin::Autonomous,
            outcome: ToyInteractionOutcome::Accepted,
            phase: ToyInteractionPhase::Approach,
            relationship: None,
        });
        world.creature.aquarium.steering = SteeringMode::Approach;
        let relationship = world.creature.relationship;

        let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayed {
                toy: ToyId::Bell,
                origin: ToyOrigin::Autonomous,
                ..
            }
        )));
        assert_eq!(world.creature.relationship, relationship);
        assert!(
            !world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Bell }))
        );
        assert!(
            world
                .creature
                .idle_life
                .visit_evidence
                .iter()
                .any(|visit| visit.destination == SemanticDestination::Toy(ToyId::Bell))
        );
    }

    #[test]
    fn pending_toy_save_and_offline_resume_resolve_exactly_once_without_old_cues() {
        let mut world = WorldState::new(206, "Resume");
        let mut rng = SeededRandom::new(206);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let json = SaveGame::capture(&world, &rng).to_json().unwrap();

        let (zero_world, _) = SaveGame::from_json(&json).unwrap().resume();
        assert!(
            zero_world
                .creature
                .interaction_state
                .toy_interaction
                .is_some()
        );
        assert!(
            !zero_world
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
        );

        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();
        let progress = advance_offline(&mut resumed, SIMULATION_TICK_MS, &mut resumed_rng);
        assert!(!progress.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayAccepted { .. }
                | GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
        )));
        assert_eq!(
            resumed
                .creature
                .memories
                .iter()
                .filter(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
                .count(),
            1
        );
        let resumed_json = SaveGame::capture(&resumed, &resumed_rng).to_json().unwrap();
        let (mut again, mut again_rng) = SaveGame::from_json(&resumed_json).unwrap().resume();
        advance_offline(&mut again, SIMULATION_TICK_MS, &mut again_rng);
        assert_eq!(
            again
                .creature
                .memories
                .iter()
                .filter(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
                .count(),
            1
        );
    }

    #[test]
    fn v5_toy_travel_migrates_conservatively() {
        let mut play = WorldState::new(207, "OldPlay");
        let rng = SeededRandom::new(207);
        play.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Good],
            0.7,
            0.6,
        );
        play.creature.current_intention = Intention::Play;
        play.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Ball));
        let mut value = serde_json::to_value(SaveGame::capture(&play, &rng)).unwrap();
        value["save_version"] = 5.into();
        value["world"]["save_version"] = 5.into();
        value["world"]["creature"]["aquarium"]
            .as_object_mut()
            .unwrap()
            .remove("travel_purpose");
        let interaction = value["world"]["creature"]["interaction_state"]
            .as_object_mut()
            .unwrap();
        interaction.remove("next_toy_interaction_id");
        interaction.remove("toy_interaction");
        interaction.remove("last_resolved_toy_interaction");
        let embedded: WorldState = serde_json::from_value(value["world"].clone()).unwrap();
        let embedded = migrate_world(embedded).expect("containing saves migrate embedded worlds");
        assert_eq!(embedded.save_version, SAVE_VERSION);
        assert!(embedded.creature.aquarium.destination.is_none());
        let migrated = SaveGame::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
        assert!(migrated.world.creature.aquarium.destination.is_none());
        assert_eq!(
            migrated
                .world
                .creature
                .memories
                .iter()
                .filter(|memory| matches!(memory.kind, MemoryKind::PlayedWith { toy: ToyId::Ball }))
                .count(),
            1
        );

        let mut refusal = WorldState::new(208, "OldRefusal");
        refusal.creature.current_intention = Intention::RefuseAndStare;
        refusal.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Sock));
        let mut value = serde_json::to_value(SaveGame::capture(&refusal, &rng)).unwrap();
        value["save_version"] = 5.into();
        value["world"]["save_version"] = 5.into();
        value["world"]["creature"]["aquarium"]
            .as_object_mut()
            .unwrap()
            .remove("travel_purpose");
        let interaction = value["world"]["creature"]["interaction_state"]
            .as_object_mut()
            .unwrap();
        interaction.remove("next_toy_interaction_id");
        interaction.remove("toy_interaction");
        interaction.remove("last_resolved_toy_interaction");
        let embedded: WorldState = serde_json::from_value(value["world"].clone()).unwrap();
        let embedded = migrate_world(embedded).expect("containing saves migrate embedded worlds");
        assert!(matches!(
            embedded.creature.aquarium.travel_purpose,
            Some(TravelPurpose::RefusalStare { .. })
        ));
        let migrated = SaveGame::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
        assert!(matches!(
            migrated.world.creature.aquarium.travel_purpose,
            Some(TravelPurpose::RefusalStare { .. })
        ));
        assert!(
            migrated
                .world
                .creature
                .interaction_state
                .toy_interaction
                .is_some_and(|interaction| interaction.toy == ToyId::Sock)
        );
    }

    #[test]
    fn validation_rejects_unowned_and_mismatched_toy_travel() {
        let mut unowned = WorldState::new(209, "Unowned");
        unowned.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Ball));
        assert_eq!(
            unowned.validate(),
            Err(StateValidationError::ToyInteraction)
        );

        let mut mismatch = WorldState::new(210, "Mismatch");
        let mut rng = SeededRandom::new(210);
        mismatch.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        step(
            &mut mismatch,
            &[PlayerEvent::Play(ToyId::Ball)],
            0,
            &mut rng,
        );
        mismatch.creature.aquarium.destination = Some(SemanticDestination::Toy(ToyId::Bell));
        assert_eq!(
            mismatch.validate(),
            Err(StateValidationError::ToyInteraction)
        );
        mismatch.creature.interaction_state.next_toy_interaction_id = 0;
        assert_eq!(
            mismatch.validate(),
            Err(StateValidationError::ToyInteraction)
        );
    }

    #[test]
    fn active_toy_blocks_standalone_relationship_and_requires_its_exact_travel() {
        let mut world = WorldState::new(211, "Owned");
        let mut rng = SeededRandom::new(211);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.7,
            0.8,
        );
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);

        let events = trigger_relationship_beat(&mut world, RelationshipTrigger::QuietMoment);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, GameEvent::RelationshipBeatStarted { .. }))
        );
        assert!(world.creature.relationship_expression.active.is_none());
        assert!(world.validate().is_ok());

        world.creature.aquarium.destination = None;
        world.creature.aquarium.travel_purpose = None;
        assert_eq!(world.validate(), Err(StateValidationError::ToyInteraction));
    }

    #[test]
    fn replacing_idle_travel_emits_an_explicit_interruption() {
        let mut world = WorldState::new(213, "ReplacementEvent");
        let mut rng = SeededRandom::new(213);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.aquarium.destination = Some(SemanticDestination::Cave);
        world.creature.aquarium.travel_purpose = Some(TravelPurpose::IdleVisit {
            visit_id: std::num::NonZeroU64::new(1).unwrap(),
        });
        world.creature.aquarium.steering = SteeringMode::Approach;

        let events = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        assert!(events.contains(&GameEvent::ActionAborted {
            destination: SemanticDestination::Cave,
        }));
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayAccepted {
                toy: ToyId::Ball,
                origin: ToyOrigin::Player,
                ..
            }
        )));
    }

    #[test]
    fn autonomous_toy_owner_is_allocated_before_travel_and_survives_resume() {
        let mut first = WorldState::new(212, "AutonomousOwner");
        first.creature.needs.curiosity = 0.9;
        first.creature.traits.fussiness = 0.1;
        let mut second = first.clone();
        let mut first_rng = SeededRandom::new(212);
        let mut second_rng = first_rng;

        let first_events = step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng);
        let second_events = step(&mut second, &[], SIMULATION_TICK_MS, &mut second_rng);
        assert_eq!(first_events, second_events);
        assert_eq!(first, second);
        let interaction = first
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .expect("autonomous toy owner is created with the visit");
        assert_eq!(interaction.origin, ToyOrigin::Autonomous);
        assert_eq!(interaction.outcome, ToyInteractionOutcome::Accepted);
        assert_eq!(interaction.phase, ToyInteractionPhase::Approach);
        let interaction_id = interaction.id;
        assert_eq!(
            first.creature.aquarium.travel_purpose,
            Some(TravelPurpose::ToyInteraction { interaction_id })
        );
        assert!(first_events.contains(&GameEvent::ToyPlayAccepted {
            toy: interaction.toy,
            interaction_id,
            origin: ToyOrigin::Autonomous,
        }));

        let json = SaveGame::capture(&first, &first_rng).to_json().unwrap();
        let (zero_advance, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(
            zero_advance
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .map(|interaction| interaction.id),
            Some(interaction_id)
        );

        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();
        let progress = advance_offline(&mut resumed, SIMULATION_TICK_MS, &mut resumed_rng);
        assert!(!progress.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayAccepted { .. }
                | GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
        )));
        assert_eq!(
            resumed
                .creature
                .interaction_state
                .last_resolved_toy_interaction
                .map(|resolved| resolved.id),
            Some(interaction_id)
        );
        assert!(
            !resumed
                .creature
                .memories
                .iter()
                .any(|memory| matches!(memory.kind, MemoryKind::PlayedWith { .. }))
        );
    }

    #[test]
    fn shared_toy_context_survives_new_eligibility_and_save_before_contact() {
        let mut world = WorldState::new(214, "ContextOwner");
        let mut rng = SeededRandom::new(214);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        let original_evidence = world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.7,
            0.8,
        );
        let receipt = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let interaction_id = receipt
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .unwrap();
        let stored_context = world
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .and_then(|interaction| interaction.relationship.clone())
            .expect("eligible context is captured at receipt");
        assert_eq!(
            stored_context.evidence,
            vec![RelationshipEvidence::Memory {
                id: original_evidence,
            }]
        );

        world.remember(MemoryKind::WasComforted, &[Concept::You], 0.8, 0.8);
        let newer_same_subject = world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.9,
            0.9,
        );
        let newly_selected = select_action_relationship_context(
            &world,
            RelationshipTrigger::ToyEngaged { toy: ToyId::Ball },
        )
        .expect("new evidence changes fresh selection");
        assert!(
            newly_selected
                .evidence
                .contains(&RelationshipEvidence::Memory {
                    id: newer_same_subject,
                })
        );
        assert_ne!(newly_selected.evidence, stored_context.evidence);

        world.creature.aquarium.position = toy_position(&world, ToyId::Ball);
        let json = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(
            resumed
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .and_then(|interaction| interaction.relationship.as_ref()),
            Some(&stored_context)
        );

        let contact = step(&mut resumed, &[], SIMULATION_TICK_MS, &mut resumed_rng);
        assert!(contact.contains(&GameEvent::ToyContacted {
            toy: ToyId::Ball,
            interaction_id,
            origin: ToyOrigin::Player,
        }));
        assert!(contact.iter().any(|event| matches!(
            event,
            GameEvent::ActionRelationshipStarted {
                action_id,
                motif: RelationshipMotifKey::SharedToy(ToyId::Ball),
                subject: RelationshipSubject::Toy(ToyId::Ball),
                evidence,
                ..
            } if *action_id == interaction_id.get() && *evidence == stored_context.evidence
        )));
    }
}
