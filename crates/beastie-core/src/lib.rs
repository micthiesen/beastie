//! Authoritative, deterministic creature simulation.

mod language;
mod lexicon;
mod memory;
mod model;
mod random;
mod relationship;
mod save;
mod simulation;
mod teaching;
mod wants;

pub use language::{
    GroundedUtterance, UtteranceInterpretation, UtteranceReference, ground_utterance,
    interpret_utterance,
};
pub use lexicon::{
    ActWord, FocusMark, Hearing, LEARN_EVIDENCE, LEARN_HEARINGS, Lexicon, Meaning, Salience,
    WordKnowledge, content_words, echo_attempt, mark_focus,
};
pub use memory::{MemoryCue, MemoryQuery, select_candidate_memories};
pub use model::{
    ActionPhase, ActionRelationshipContext, ActionRelationshipMoment, ActionTimeline,
    ActivityInterruptionOwner, ActivityPhase, ActivityPurpose, ActivityRecipe,
    ActivitySelectionEvidence, ActivitySubject, AquariumCreatureState, AquariumPosition,
    AquariumState, Belief, BeliefId, BeliefKind, Concept, ConversationState, Creature, DepthLane,
    Development, DevelopmentMilestone, ExpressedMotif, Facing, FoodBuoyancy, FoodDisposition,
    FoodDropRejectionReason, FoodId, FoodObject, FoodOutcome, GazeTarget, Idiolect, IdiolectQuirk,
    IdleLifeState, InitiatedBehavior, InitiativeReason, Intention, InteractionCounters,
    InteractionState, LanguageExposure, LanguageStage, Memory, MemoryId, MemoryKind, Mood,
    NamingTarget, Needs, NonverbalAct, NormalizedPosition, NormalizedVelocity, PrivateLifeActivity,
    PrivateLifeKind, PrivateLifeState, Reaction, RecentActivity, Relationship, RelationshipBeat,
    RelationshipBeatPhase, RelationshipEvidence, RelationshipExpressionKind,
    RelationshipExpressionMode, RelationshipExpressionState, RelationshipMotif,
    RelationshipMotifKey, RelationshipPerformanceRecipe, RelationshipPerformanceRecord,
    RelationshipSubject, RelationshipTrigger, RelationshipTriggerKind, ResolvedToyInteraction,
    Routine, SemanticDestination, SocialAct, SocialHabits, StateValidationError, SteeringMode,
    ToyId, ToyInteraction, ToyInteractionOutcome, ToyInteractionPhase, ToyObjectState, ToyOrigin,
    ToyResponse, Traits, TravelPurpose, TravelTarget, VisitEvidence, WorldObject, WorldState,
};
pub use random::{RandomDomain, RandomSource, SeededRandom, deterministic_unit};
pub use relationship::{
    derive_relationship_motifs, performance_recipe_for, select_action_relationship_context,
    select_relationship_beat,
};
pub use save::{SaveError, SaveGame, migrate_world};
pub use simulation::{
    DialogueActionOwner, DialogueHandoff, DialogueHandoffState, GameEvent, MAX_OFFLINE_MS,
    OfflineProgress, PlayerEvent, SIMULATION_TICK_MS, SOCK_RELEASE_SPEED, SpeechAttention,
    TALK_COOLDOWN_MS, advance_offline, apply_grounded_utterance, approach_position, bubble_point,
    destination_position, dialogue_handoff, feeding_position, held_toy_position, movement_target,
    speech_attention, step, trigger_relationship_beat,
};
pub use teaching::{RequestResponse, current_salience};
pub use wants::{Want, current_want, engaged_toy};

pub const SAVE_VERSION: u32 = 7;
pub const ACTIVE_DAY_MS: u64 = 15 * 60_000;
pub const RELATIONSHIP_EXPRESSION_SCHEMA_VERSION: u32 = 3;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Pointer motion is attention: it must never produce any of these events.
    fn is_interruption_or_intention_change(event: &GameEvent) -> bool {
        matches!(
            event,
            GameEvent::IntentionChanged { .. }
                | GameEvent::ActionAborted { .. }
                | GameEvent::ToyInteractionInterrupted { .. }
                | GameEvent::PrivateLifeInterrupted { .. }
                | GameEvent::ActionRelationshipInterrupted { .. }
                | GameEvent::RelationshipBeatInterrupted(_)
        )
    }

    #[test]
    fn stationary_food_phases_clear_old_swim_velocity_without_moving() {
        let mut world = WorldState::new(7, "Stopped");
        let mut rng = SeededRandom::new(7);
        let position = world.creature.aquarium.position;
        world.creature.aquarium.velocity = NormalizedVelocity { x: 650, y: -400 };
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(8_000, 3_000),
            }],
            0,
            &mut rng,
        );
        assert_eq!(world.creature.aquarium.position, position);
        assert_eq!(
            world.creature.aquarium.velocity,
            NormalizedVelocity::default()
        );
        for phase in [
            ActionPhase::Notice,
            ActionPhase::Brake,
            ActionPhase::Gaze,
            ActionPhase::Turn,
            ActionPhase::Inspect,
            ActionPhase::Act,
            ActionPhase::Recover,
        ] {
            world.creature.aquarium.action.as_mut().unwrap().phase = phase;
            world.creature.aquarium.velocity = NormalizedVelocity { x: 650, y: -400 };
            assert!(step(&mut world, &[], 0, &mut rng).is_empty());
            assert_eq!(world.creature.aquarium.position, position);
            assert_eq!(
                world.creature.aquarium.velocity,
                NormalizedVelocity::default()
            );
        }
        world.creature.aquarium.action.as_mut().unwrap().phase = ActionPhase::Approach;
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert_ne!(world.creature.aquarium.position, position);
        assert_ne!(
            world.creature.aquarium.velocity,
            NormalizedVelocity::default()
        );
    }

    #[test]
    fn sleeping_creature_clears_old_swim_velocity_and_keeps_its_position() {
        let mut world = WorldState::new(7, "Sleeping");
        let mut rng = SeededRandom::new(7);
        world.creature.current_intention = Intention::Sleep;
        world.creature.needs.energy = 0.2;
        world.creature.interaction_state.sleep_started_at_ms = Some(0);
        let position = world.creature.aquarium.position;
        for dt in [0, SIMULATION_TICK_MS, 5 * SIMULATION_TICK_MS] {
            world.creature.aquarium.velocity = NormalizedVelocity { x: 650, y: -400 };
            step(&mut world, &[], dt, &mut rng);
            assert_eq!(world.creature.current_intention, Intention::Sleep);
            assert_eq!(world.creature.aquarium.position, position);
            assert_eq!(
                world.creature.aquarium.velocity,
                NormalizedVelocity::default()
            );
        }
    }

    #[test]
    fn food_phase_velocity_is_identical_for_batched_and_split_ticks() {
        let mut world = WorldState::new(7, "Tick chunks");
        let mut rng = SeededRandom::new(7);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(8_000, 3_000),
            }],
            0,
            &mut rng,
        );
        for ticks in 1..=20 {
            let mut bulk = world.clone();
            let mut split = world.clone();
            let mut bulk_rng = rng;
            let mut split_rng = rng;
            let bulk_events = step(&mut bulk, &[], ticks * SIMULATION_TICK_MS, &mut bulk_rng);
            let mut split_events = Vec::new();
            for _ in 0..ticks {
                split_events.extend(step(&mut split, &[], SIMULATION_TICK_MS, &mut split_rng));
            }
            assert_eq!(bulk, split, "after {ticks} ticks");
            assert_eq!(bulk_events, split_events, "after {ticks} ticks");
        }
    }

    #[test]
    fn saved_recovery_with_stale_velocity_is_independent_of_partial_tick_steps() {
        let mut world = WorldState::new(7, "Recovered save");
        let mut rng = SeededRandom::new(7);
        step(
            &mut world,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(8_000, 3_000),
            }],
            0,
            &mut rng,
        );
        for _ in 0..30 {
            if world
                .creature
                .aquarium
                .action
                .as_ref()
                .is_some_and(|action| action.phase == ActionPhase::Recover)
            {
                break;
            }
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }
        let action = world.creature.aquarium.action.as_mut().unwrap();
        assert_eq!(action.phase, ActionPhase::Recover);
        action.elapsed_ms = action.phase_duration_ms.saturating_sub(SIMULATION_TICK_MS);
        world.creature.aquarium.velocity = NormalizedVelocity { x: 650, y: 400 };
        let json = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (mut bulk, mut bulk_rng) = SaveGame::from_json(&json).unwrap().resume();
        let (mut split, mut split_rng) = SaveGame::from_json(&json).unwrap().resume();
        let bulk_events = step(&mut bulk, &[], SIMULATION_TICK_MS, &mut bulk_rng);
        let mut split_events = step(&mut split, &[], SIMULATION_TICK_MS / 2, &mut split_rng);
        split_events.extend(step(
            &mut split,
            &[],
            SIMULATION_TICK_MS / 2,
            &mut split_rng,
        ));
        assert!(bulk.creature.aquarium.action.is_none());
        assert_eq!(bulk, split);
        assert_eq!(bulk_events, split_events);
        assert_eq!(
            bulk.creature.aquarium.velocity,
            NormalizedVelocity::default()
        );
    }

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
            recovery_until_ms: 0,
            rounds_left: 0,
            contacts: 0,
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
            SIMULATION_TICK_MS * 15 + SIMULATION_TICK_MS / 2,
            &mut rng,
        );
        assert_eq!(a.simulation_remainder_ms, SIMULATION_TICK_MS / 2);
        assert!(a.creature.aquarium.action.is_some());
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

        // Three simulated minutes.
        for _ in 0..(180_000 / SIMULATION_TICK_MS) {
            let first_events = step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng);
            let replay_events = step(&mut replay, &[], SIMULATION_TICK_MS, &mut replay_rng);
            assert_eq!(first_events, replay_events);
            autonomous_toy_arrivals += first_events
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyObjectResponded { .. }))
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
            (1_500..=4_500).contains(duration) && duration % SIMULATION_TICK_MS == 0
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
        // Shorter idle bouts mean the creature may be settled at this instant; it must resume
        // its own life within a bounded quiet span.
        let mut resumed_life = false;
        for _ in 0..(20_000 / SIMULATION_TICK_MS) {
            if world.creature.aquarium.destination.is_some()
                || world.creature.private_life.active.is_some()
            {
                resumed_life = true;
                break;
            }
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }
        assert!(resumed_life, "comforted creature returns to its own life");
    }

    #[test]
    fn affection_expiry_preserves_new_private_travel_and_saved_completion() {
        let mut world = WorldState::new(42, "Continuing");
        let mut rng = SeededRandom::new(42);
        // The native dialogue-races sequence: a first private notice, Talk, then Comfort.
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        step(&mut world, &[PlayerEvent::Talk], 0, &mut rng);
        step(&mut world, &[PlayerEvent::Comfort], 0, &mut rng);
        // The comfort journey arrives, then a private activity starts travelling.
        for _ in 0..(10_000 / SIMULATION_TICK_MS) {
            if world
                .creature
                .private_life
                .active
                .as_ref()
                .is_some_and(|activity| {
                    activity.phase == ActivityPhase::Approach
                        && world.creature.aquarium.destination.is_some()
                })
            {
                break;
            }
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }
        let activity = world.creature.private_life.active.as_ref().unwrap().clone();
        assert_eq!(activity.kind, PrivateLifeKind::PlantInspect);
        assert_eq!(activity.phase, ActivityPhase::Approach);
        assert_eq!(world.creature.current_intention, Intention::ShowAffection);
        // Short private journeys finish well inside the affection span, so expire the old
        // affection timer on the next tick while the newer private travel owns locomotion.
        world.creature.interaction_state.affectionate_until_ms =
            world.elapsed_ms + SIMULATION_TICK_MS;
        let owner = Some(TravelPurpose::PrivateLife {
            activity_id: activity.id,
        });
        assert_eq!(world.creature.aquarium.travel_purpose, owner);
        let json = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();

        let expiry = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert_eq!(
            expiry,
            step(&mut resumed, &[], SIMULATION_TICK_MS, &mut resumed_rng)
        );
        assert_eq!(world, resumed);
        assert_eq!(world.creature.current_intention, Intention::Idle);
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Plant)
        );
        assert_eq!(world.creature.aquarium.travel_purpose, owner);
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Approach);
        assert_eq!(
            dialogue_handoff(&world),
            DialogueHandoff {
                owner: Some(DialogueActionOwner::PrivateLife(activity.id)),
                state: DialogueHandoffState::WaitingForContact,
            }
        );

        let continuation_ticks = 80;
        let mut continuation = Vec::new();
        for _ in 0..continuation_ticks {
            continuation.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
        }
        assert_eq!(
            continuation,
            step(
                &mut resumed,
                &[],
                SIMULATION_TICK_MS * continuation_ticks,
                &mut resumed_rng
            )
        );
        assert_eq!(world, resumed);
        assert_eq!(continuation.iter().filter(|event| matches!(event,
            GameEvent::PrivateLifeCompleted { activity_id, .. } if *activity_id == activity.id
        )).count(), 1);
        assert!(!continuation.iter().any(|event| matches!(event,
            GameEvent::PrivateLifeInterrupted { activity_id, .. } if *activity_id == activity.id
        )));
        assert_ne!(
            dialogue_handoff(&world).owner,
            Some(DialogueActionOwner::PrivateLife(activity.id))
        );
    }

    #[test]
    fn affection_expiry_still_releases_its_own_unfinished_journey() {
        let mut world = WorldState::new(42, "EnoughAffection");
        let mut rng = SeededRandom::new(42);
        world.creature.aquarium.position = NormalizedPosition::new(0, 10_000);
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(NormalizedPosition::new(
                10_000, 0,
            )))],
            0,
            &mut rng,
        );
        step(&mut world, &[PlayerEvent::Comfort], 0, &mut rng);
        step(&mut world, &[], SIMULATION_TICK_MS * 2, &mut rng);
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Player)
        );
        assert_eq!(world.creature.current_intention, Intention::ShowAffection);
        // The swim to the player is now much shorter than the affection span; expire the
        // timer while the comfort journey is still unfinished.
        world.creature.interaction_state.affectionate_until_ms =
            world.elapsed_ms + SIMULATION_TICK_MS;

        let expiry = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(expiry.contains(&GameEvent::IntentionChanged {
            from: Intention::ShowAffection,
            to: Intention::Idle,
        }));
        assert_eq!(world.creature.aquarium.destination, None);
        assert_eq!(world.creature.aquarium.travel_purpose, None);
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Hover);
    }

    #[test]
    fn cursor_motion_during_affection_never_replaces_the_comfort_journey() {
        let mut world = WorldState::new(42, "Following");
        let mut rng = SeededRandom::new(42);
        world.creature.relationship.trust = 0.8;
        world.creature.traits.sociability = 0.8;
        step(&mut world, &[PlayerEvent::Comfort], 0, &mut rng);
        step(&mut world, &[], SIMULATION_TICK_MS * 2, &mut rng);
        let owner = world.creature.aquarium.travel_purpose;
        assert!(matches!(owner, Some(TravelPurpose::CursorSocial { .. })));
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Player)
        );
        let near = world.creature.aquarium.position;
        for cursor in [
            NormalizedPosition::new(10_000, 10_000),
            near,
            NormalizedPosition::new(near.x + 400, near.y - 300),
        ] {
            let events = step(
                &mut world,
                &[PlayerEvent::Cursor(Some(cursor))],
                0,
                &mut rng,
            );
            assert!(!events.iter().any(is_interruption_or_intention_change));
            assert_eq!(world.aquarium.cursor, Some(cursor));
            assert_eq!(world.creature.current_intention, Intention::ShowAffection);
            assert_eq!(
                world.creature.aquarium.destination,
                Some(SemanticDestination::Player)
            );
            assert_eq!(world.creature.aquarium.travel_purpose, owner);
            assert_eq!(world.creature.aquarium.gaze, GazeTarget::Player);
        }
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
    fn cursor_never_starts_travel_even_with_sustained_care() {
        let mut world = WorldState::new(45, "Trust");
        world.creature.traits.sociability = 1.0;
        world.creature.relationship.trust = 0.9;
        let mut rng = SeededRandom::new(45);
        let position = world.creature.aquarium.position;
        for cursor in [
            NormalizedPosition::new(8_000, 4_500),
            NormalizedPosition::new(position.x + 600, position.y),
            NormalizedPosition::new(0, 0),
        ] {
            let events = step(
                &mut world,
                &[PlayerEvent::Cursor(Some(cursor))],
                0,
                &mut rng,
            );
            assert!(!events.iter().any(is_interruption_or_intention_change));
            assert_ne!(world.creature.aquarium.steering, SteeringMode::Approach);
            assert_eq!(world.creature.aquarium.destination, None);
            assert_eq!(world.creature.aquarium.travel_purpose, None);
        }
        // Holding a cursor in the tank never turns into a player-following journey.
        for _ in 0..(20_000 / SIMULATION_TICK_MS) {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            assert_ne!(world.creature.current_intention, Intention::ApproachPlayer);
            assert!(!matches!(
                world.creature.aquarium.travel_purpose,
                Some(TravelPurpose::CursorSocial { .. })
            ));
        }
    }

    #[test]
    fn sleep_restores_energy_and_initiative_clears_or_expires() {
        let mut world = WorldState::new(46, "Rest");
        world.creature.needs.energy = 0.09;
        let mut rng = SeededRandom::new(46);
        let mut events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(events.contains(&GameEvent::SleepStarted));
        // Sleep restores energy per second; ten seconds is enough to wake rested.
        for _ in 0..(10_000 / SIMULATION_TICK_MS) {
            events.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
        }
        assert!(events.contains(&GameEvent::SleepEnded));
        assert_ne!(world.creature.current_intention, Intention::Sleep);
        assert!(world.creature.needs.energy >= 0.68);

        // An unoccupied, hungry creature asks; satisfying the need clears the request, and it
        // does not nag again until its asking interval has passed.
        world.creature.private_life.active = None;
        world.creature.idle_life.settled_until_ms = world.elapsed_ms + 600_000;
        world.creature.needs.hunger = 0.9;
        let asked = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        assert!(asked.contains(&GameEvent::InitiatedTalk(InitiativeReason::Hunger)));
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
        let mut asked_again_at = None;
        for _ in 0..(60_000 / SIMULATION_TICK_MS) {
            world.creature.needs.hunger = 0.9;
            let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            if events.contains(&GameEvent::InitiatedTalk(InitiativeReason::Hunger)) {
                asked_again_at = Some(world.elapsed_ms);
                break;
            }
        }
        let asked_again_at = asked_again_at.expect("asks again after its interval");
        assert!(asked_again_at.saturating_sub(first_request) >= 45_000);
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
    fn motivated_creature_asks_for_company_and_leans_in() {
        let mut a = WorldState::new(12, "Ask");
        a.creature.needs.comfort = 0.1;
        a.creature.traits.sociability = 1.0;
        a.creature.idle_life.settled_until_ms = 600_000;
        let mut rng = SeededRandom::new(12);
        let events = step(&mut a, &[], 1_000, &mut rng);
        assert!(events.contains(&GameEvent::InitiatedTalk(InitiativeReason::Loneliness)));
        assert_eq!(
            a.creature
                .initiated_behavior
                .as_ref()
                .and_then(|ask| ask.nonverbal),
            Some(NonverbalAct::LeanAgainstPlayer)
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
    fn pointer_motion_never_interrupts_an_accepted_toy_interaction() {
        let mut world = WorldState::new(213, "Pointer");
        let mut rng = SeededRandom::new(213);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.aquarium.position = NormalizedPosition::new(1_000, 2_000);
        let receipt = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let interaction_id = receipt
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                _ => None,
            })
            .expect("play is accepted");
        let gaze = world.creature.aquarium.gaze;
        assert_eq!(gaze, GazeTarget::Toy(ToyId::Ball));
        let mut contacted = false;
        for tick in 0..(10_000 / SIMULATION_TICK_MS) as i32 {
            // A restless pointer: sweeping, hovering right over the creature, leaving.
            let position = world.creature.aquarium.position;
            let cursor = match tick % 4 {
                0 => Some(NormalizedPosition::new(position.x + 150, position.y - 100)),
                1 => Some(NormalizedPosition::new(
                    (tick * 731) % 10_000,
                    (tick * 389) % 10_000,
                )),
                2 => Some(position),
                _ => None,
            };
            let events = step(
                &mut world,
                &[PlayerEvent::Cursor(cursor)],
                SIMULATION_TICK_MS,
                &mut rng,
            );
            assert!(
                !events.iter().any(is_interruption_or_intention_change),
                "pointer motion interrupted play at tick {tick}: {events:?}"
            );
            if events.contains(&GameEvent::ToyContacted {
                toy: ToyId::Ball,
                interaction_id,
                origin: ToyOrigin::Player,
            }) {
                contacted = true;
                break;
            }
            assert_eq!(
                world
                    .creature
                    .interaction_state
                    .toy_interaction
                    .as_ref()
                    .map(|interaction| interaction.id),
                Some(interaction_id)
            );
            assert_eq!(
                world.creature.aquarium.travel_purpose,
                Some(TravelPurpose::ToyInteraction { interaction_id })
            );
            assert_eq!(
                world.creature.aquarium.gaze, gaze,
                "the pointer never steals an occupied creature's gaze"
            );
        }
        assert!(contacted, "the accepted toy is eventually contacted");
    }

    #[test]
    fn cursor_is_persisted_draws_idle_gaze_and_resentment_flees() {
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
        let json = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (resumed, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(resumed.aquarium.cursor, world.aquarium.cursor);
        // A distant pointer neither draws gaze nor starts travel.
        assert_ne!(world.creature.aquarium.gaze, GazeTarget::Cursor);
        assert_ne!(world.creature.aquarium.steering, SteeringMode::Approach);
        assert_eq!(world.creature.aquarium.destination, None);

        // A nearby pointer catches an unoccupied creature's eye without moving it.
        let position = world.creature.aquarium.position;
        let near = NormalizedPosition::new(position.x + 500, position.y - 300);
        step(&mut world, &[PlayerEvent::Cursor(Some(near))], 0, &mut rng);
        assert_eq!(world.creature.aquarium.gaze, GazeTarget::Cursor);
        assert_ne!(world.creature.aquarium.steering, SteeringMode::Approach);
        assert_eq!(world.creature.aquarium.destination, None);
        // Moving away again releases the glance.
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(NormalizedPosition::new(0, 0)))],
            0,
            &mut rng,
        );
        assert_ne!(world.creature.aquarium.gaze, GazeTarget::Cursor);

        // A resentful creature flees a nearby pointer, then calms once it has distance.
        world.creature.relationship.resentment = 0.9;
        step(&mut world, &[PlayerEvent::Cursor(Some(near))], 0, &mut rng);
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Flee);
        let distance = |world: &WorldState| -> i32 {
            let position = world.creature.aquarium.position;
            (position.x - near.x).abs() + (position.y - near.y).abs()
        };
        let start = distance(&world);
        let mut calmed = false;
        for _ in 0..(5_000 / SIMULATION_TICK_MS) {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            if world.creature.aquarium.steering != SteeringMode::Flee {
                calmed = true;
                break;
            }
        }
        assert!(calmed, "flight ends once the creature has distance");
        assert!(distance(&world) > start);
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
        let mut approach_ticks = 0;
        for _ in 0..(10_000 / SIMULATION_TICK_MS) {
            let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::FoodConsumed(_) | GameEvent::FoodRejected(_)
            )));
            let phase = world
                .creature
                .aquarium
                .action
                .as_ref()
                .map(|action| action.phase);
            match phase {
                Some(ActionPhase::Approach) => approach_ticks += 1,
                Some(ActionPhase::Inspect) => break,
                _ => {}
            }
        }
        assert!(approach_ticks > 1, "feeding includes a visible swim");
        assert_eq!(
            world
                .creature
                .aquarium
                .action
                .as_ref()
                .map(|action| action.phase),
            Some(ActionPhase::Inspect)
        );
        assert!(distance(&world) < initial_distance);
    }

    #[test]
    fn cursor_flee_moves_until_its_stop_threshold() {
        let mut world = WorldState::new(91, "Cursor");
        let mut rng = SeededRandom::new(91);
        world.creature.relationship.trust = 0.9;
        world.creature.traits.sociability = 0.9;
        world.creature.relationship.resentment = 0.9;
        let position = world.creature.aquarium.position;
        let flee_cursor = NormalizedPosition::new(position.x + 1_000, position.y);
        step(
            &mut world,
            &[PlayerEvent::Cursor(Some(flee_cursor))],
            0,
            &mut rng,
        );
        assert_eq!(world.creature.aquarium.steering, SteeringMode::Flee);
        let before_flee = (world.creature.aquarium.position.x - flee_cursor.x).abs();
        for _ in 0..3 {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }
        assert!((world.creature.aquarium.position.x - flee_cursor.x).abs() > before_flee);
        for _ in 0..(5_000 / SIMULATION_TICK_MS) {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        }
        assert_ne!(world.creature.aquarium.steering, SteeringMode::Flee);
    }

    #[test]
    fn needs_traits_routines_and_favorites_choose_distinct_destinations() {
        // Private activities notice before they travel, so wait for the first chosen journey.
        fn first_destination(
            world: &mut WorldState,
            rng: &mut SeededRandom,
        ) -> Option<SemanticDestination> {
            for _ in 0..(5_000 / SIMULATION_TICK_MS) {
                step(world, &[], SIMULATION_TICK_MS, rng);
                if world.creature.aquarium.destination.is_some() {
                    break;
                }
            }
            world.creature.aquarium.destination
        }

        let mut hungry = WorldState::new(92, "Hungry");
        hungry.creature.needs.hunger = 0.9;
        hungry.creature.idle_life.last_arrived_destination = Some(SemanticDestination::Bottom);
        let mut rng = SeededRandom::new(92);
        assert_eq!(
            first_destination(&mut hungry, &mut rng),
            Some(SemanticDestination::Bottom)
        );

        let mut fussy = WorldState::new(93, "Fussy");
        fussy.creature.needs.curiosity = 0.9;
        fussy.creature.traits.fussiness = 0.9;
        // Without a favorite toy pulling it away, fussiness favors the plant.
        fussy.creature.toy_preferences.clear();
        let mut fussy_rng = SeededRandom::new(93);
        assert_eq!(
            first_destination(&mut fussy, &mut fussy_rng),
            Some(SemanticDestination::Plant)
        );

        let mut routine = WorldState::new(94, "Routine");
        routine.set_routine(Routine {
            hour_start: 0,
            destination: SemanticDestination::Cave,
            strength: 3,
        });
        let mut routine_rng = SeededRandom::new(94);
        assert_eq!(
            first_destination(&mut routine, &mut routine_rng),
            Some(SemanticDestination::Cave)
        );
        routine.creature.routines.clear();
        routine.record_favorite(SemanticDestination::Plant);
        routine.elapsed_ms = 7_000;
        routine.creature.aquarium.destination = None;
        routine.creature.aquarium.travel_purpose = None;
        routine.creature.private_life.active = None;
        assert_eq!(
            first_destination(&mut routine, &mut routine_rng),
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
        let mut played_toy = Vec::new();
        for _ in 0..(10_000 / SIMULATION_TICK_MS) {
            played_toy.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
            if played_toy
                .iter()
                .any(|event| matches!(event, GameEvent::ToyPlayed { .. }))
            {
                break;
            }
        }
        assert!(played_toy.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayed {
                toy: ToyId::Bell,
                origin: ToyOrigin::Player,
                ..
            }
        )));
        assert_eq!(world.creature.current_intention, Intention::Play);
        assert!(
            world.creature.aquarium.gaze == GazeTarget::Toy(ToyId::Bell)
                || world.creature.private_life.active.is_some()
        );
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
            0,
            &mut first_rng,
        );
        // Step into the swim, then a little further so the save lands mid approach.
        for _ in 0..(5_000 / SIMULATION_TICK_MS) {
            if first
                .creature
                .aquarium
                .action
                .as_ref()
                .is_some_and(|action| action.phase == ActionPhase::Approach)
            {
                break;
            }
            step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng);
        }
        step(&mut first, &[], SIMULATION_TICK_MS * 2, &mut first_rng);
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
    fn new_memory_waits_for_a_later_quiet_relationship_opportunity() {
        let mut world = WorldState::new(104, "LaterGrounding");
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.7,
            0.7,
        );

        assert!(select_relationship_beat(&world, RelationshipTrigger::QuietMoment).is_none());
        world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS - 1;
        assert!(select_relationship_beat(&world, RelationshipTrigger::QuietMoment).is_none());
        world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS;
        assert!(matches!(
            select_relationship_beat(&world, RelationshipTrigger::QuietMoment),
            Some(RelationshipBeat {
                motif: RelationshipMotifKey::SharedToy(ToyId::Ball),
                subject: Some(RelationshipSubject::Toy(ToyId::Ball)),
                ..
            })
        ));
    }

    #[test]
    fn performance_ledger_suppresses_the_same_evidence_subject_motif_and_recipe() {
        let mut world = WorldState::new(105, "NoReceiptLoop");
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.7,
            0.7,
        );
        world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS;
        trigger_relationship_beat(&mut world, RelationshipTrigger::QuietMoment);
        let record = world
            .creature
            .relationship_expression
            .performance_ledger
            .last()
            .cloned()
            .expect("performed callback receipt");
        assert_eq!(record.motif, RelationshipMotifKey::SharedToy(ToyId::Ball));
        assert_eq!(record.subject, RelationshipSubject::Toy(ToyId::Ball));
        assert_eq!(
            record.recipe,
            RelationshipPerformanceRecipe::SharedBall(RelationshipExpressionKind::Notice)
        );

        // Isolate the authoritative exact-performance rule from the older broad motif history.
        world.creature.relationship_expression.active = None;
        world.creature.relationship_expression.recent.clear();
        world.elapsed_ms = world
            .elapsed_ms
            .saturating_add(relationship::RELATIONSHIP_PERFORMANCE_COOLDOWN_MS - 1);
        assert!(select_relationship_beat(&world, RelationshipTrigger::QuietMoment).is_none());

        world.elapsed_ms = world.elapsed_ms.saturating_add(1);
        assert!(select_relationship_beat(&world, RelationshipTrigger::QuietMoment).is_some());
    }

    #[test]
    fn performance_ledger_round_trips_without_changing_future_suppression() {
        let mut world = WorldState::new(106, "LedgerReload");
        let rng = SeededRandom::new(106);
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Bell },
            &[Concept::Toy],
            0.7,
            0.7,
        );
        world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS;
        trigger_relationship_beat(&mut world, RelationshipTrigger::QuietMoment);
        let saved = SaveGame::capture(&world, &rng)
            .to_json()
            .expect("save ledger");
        let (mut reloaded, reloaded_rng) =
            SaveGame::from_json(&saved).expect("load ledger").resume();
        assert_eq!(reloaded_rng, rng);
        assert_eq!(
            reloaded.creature.relationship_expression.performance_ledger,
            world.creature.relationship_expression.performance_ledger
        );

        reloaded.creature.relationship_expression.active = None;
        reloaded.creature.relationship_expression.recent.clear();
        reloaded.elapsed_ms = reloaded
            .elapsed_ms
            .saturating_add(relationship::RELATIONSHIP_GLOBAL_COOLDOWN_MS);
        assert!(select_relationship_beat(&reloaded, RelationshipTrigger::QuietMoment).is_none());
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
    fn early_shared_toy_notice_approaches_without_claiming_contact() {
        let mut world = WorldState::new(122, "EarlyBall");
        world.remember(
            MemoryKind::PlayedWith { toy: ToyId::Ball },
            &[Concept::Toy],
            0.6,
            0.8,
        );
        world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS;
        trigger_relationship_beat(&mut world, RelationshipTrigger::QuietMoment);
        assert_eq!(
            world
                .creature
                .relationship_expression
                .active
                .as_ref()
                .expect("shared-ball notice")
                .expression_kind,
            RelationshipExpressionKind::Notice
        );

        let mut rng = SeededRandom::new(122);
        step(&mut world, &[], 1_000, &mut rng);
        assert_eq!(
            world.creature.aquarium.destination,
            Some(SemanticDestination::Toy(ToyId::Ball))
        );
        assert!(matches!(
            world.creature.aquarium.travel_purpose,
            Some(TravelPurpose::Relationship { .. })
        ));

        let events = step(&mut world, &[], 2_000, &mut rng);
        assert_eq!(world.creature.aquarium.destination, None);
        assert_eq!(world.creature.aquarium.gaze, GazeTarget::Toy(ToyId::Ball));
        assert!(!events.iter().any(|event| matches!(
            event,
            GameEvent::ToyObjectResponded { .. } | GameEvent::ToyPlayed { .. }
        )));
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
            for _ in 0..20 {
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
    fn bottom_destination_clears_the_observed_nudged_ball() {
        let mut world = WorldState::new(42, "Forage");
        // C6 relationship footage: the previous fixed (5_000, 9_200) endpoint
        // placed the head inside this nudged ball throughout the forage.
        world
            .aquarium
            .toy_states
            .get_mut(&ToyId::Ball)
            .unwrap()
            .position = NormalizedPosition::new(4_785, 8_085);
        let bottom = destination_position(&world, SemanticDestination::Bottom).unwrap();
        assert_eq!(bottom, NormalizedPosition::new(3_665, 9_200));
        world.creature.aquarium.position = NormalizedPosition::new(5_000, 9_200);
        assert!(
            movement_target(&world).is_some(),
            "the old endpoint overlaps"
        );
        world.creature.aquarium.position = bottom;
        assert_eq!(movement_target(&world), None, "the new endpoint is clear");
        assert_eq!(
            destination_position(&world, SemanticDestination::Toy(ToyId::Ball)),
            Some(NormalizedPosition::new(4_785, 8_085))
        );
    }

    #[test]
    fn bottom_destination_checks_all_toys_and_excludes_carried_objects() {
        let mut world = WorldState::new(42, "Crowded");
        for (toy, x) in [
            (ToyId::Ball, 3_880),
            (ToyId::Bell, 5_000),
            (ToyId::Sock, 6_120),
        ] {
            world.aquarium.toy_states.get_mut(&toy).unwrap().position =
                NormalizedPosition::new(x, 9_200);
        }
        let bottom = destination_position(&world, SemanticDestination::Bottom).unwrap();
        // The nearest side of the center toy is still inside one of its neighbors.
        // The two clear outer sides tie, so select the left one deterministically.
        assert_eq!(bottom, NormalizedPosition::new(2_760, 9_200));
        world.creature.aquarium.position = bottom;
        assert_eq!(movement_target(&world), None);

        world
            .aquarium
            .toy_states
            .get_mut(&ToyId::Ball)
            .unwrap()
            .carried = true;
        assert_eq!(
            destination_position(&world, SemanticDestination::Bottom),
            Some(NormalizedPosition::new(3_880, 9_200))
        );
        for object in world.aquarium.toy_states.values_mut() {
            object.carried = true;
        }
        assert_eq!(
            destination_position(&world, SemanticDestination::Bottom),
            Some(NormalizedPosition::new(5_000, 9_200))
        );
    }

    #[test]
    fn bottom_destination_is_bounded_clear_and_independent_of_approach() {
        let columns = [0, 500, 2_500, 5_000, 7_500, 9_500, 10_000];
        for ball_x in columns {
            for bell_x in columns {
                for sock_x in columns {
                    let mut world = WorldState::new(42, "Columns");
                    for (toy, x, y) in [
                        (ToyId::Ball, ball_x, 8_085),
                        (ToyId::Bell, bell_x, 9_200),
                        (ToyId::Sock, sock_x, 10_000),
                    ] {
                        world.aquarium.toy_states.get_mut(&toy).unwrap().position =
                            NormalizedPosition::new(x, y);
                    }
                    let bottom = destination_position(&world, SemanticDestination::Bottom).unwrap();
                    assert_eq!(bottom.y, 9_200);
                    assert_eq!(bottom, bottom.clamped());
                    world.creature.aquarium.position = bottom;
                    assert_eq!(movement_target(&world), None, "blocked column {bottom:?}");
                    for start in [
                        NormalizedPosition::new(0, 0),
                        NormalizedPosition::new(10_000, 10_000),
                    ] {
                        world.creature.aquarium.position = start;
                        world.creature.aquarium.facing = Facing::Left;
                        assert_eq!(
                            destination_position(&world, SemanticDestination::Bottom),
                            Some(bottom)
                        );
                        world.creature.aquarium.facing = Facing::Right;
                        assert_eq!(
                            destination_position(&world, SemanticDestination::Bottom),
                            Some(bottom)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bottom_forage_does_not_arrive_early_inside_a_nearby_toy() {
        let mut world = WorldState::new(42, "PreciseForage");
        let mut rng = SeededRandom::new(42);
        world.creature.needs.hunger = 0.9;
        world
            .aquarium
            .toy_states
            .retain(|toy, _| *toy == ToyId::Ball);
        world
            .aquarium
            .toy_states
            .get_mut(&ToyId::Ball)
            .unwrap()
            .position = NormalizedPosition::new(5_000, 9_200);
        for _ in 0..(3_000 / SIMULATION_TICK_MS) {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            if world
                .creature
                .private_life
                .active
                .as_ref()
                .is_some_and(|activity| activity.phase == ActivityPhase::Approach)
            {
                break;
            }
        }
        let activity = world.creature.private_life.active.as_ref().unwrap();
        assert_eq!(activity.kind, PrivateLifeKind::BottomForage);
        assert_eq!(activity.phase, ActivityPhase::Approach);
        let activity_id = activity.id;

        let endpoint = NormalizedPosition::new(3_880, 9_200);
        assert_eq!(
            destination_position(&world, SemanticDestination::Bottom),
            Some(endpoint)
        );
        // Only 170 from the destination, but 950 from the ball's center. The old
        // 220-unit arrival threshold entered Act while still inside its contact radius.
        world.creature.aquarium.position = NormalizedPosition::new(4_050, 9_200);
        // Eased arrival may take a few ticks; every one of them stays in Approach until the
        // creature lands exactly on the endpoint.
        for _ in 0..(2_000 / SIMULATION_TICK_MS) {
            let approach = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            assert_eq!(
                world.creature.private_life.active.as_ref().unwrap().phase,
                ActivityPhase::Approach
            );
            assert!(!approach.iter().any(|event| matches!(
                event,
                GameEvent::PrivateLifePhaseChanged {
                    to: ActivityPhase::Act,
                    ..
                }
            )));
            if world.creature.aquarium.position == endpoint {
                break;
            }
        }
        assert_eq!(world.creature.aquarium.position, endpoint);

        let arrived = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        let activity = world.creature.private_life.active.as_ref().unwrap();
        assert_eq!(activity.phase, ActivityPhase::Act);
        assert_eq!(activity.phase_started_at_ms, world.elapsed_ms);
        assert!(!activity.payoff_reached);
        assert_eq!(
            movement_target(&world),
            None,
            "arrival must retain quiet-rest clearance"
        );
        assert_eq!(arrived.iter().filter(|event| matches!(event,
            GameEvent::PrivateLifePhaseChanged { activity_id: id, from: ActivityPhase::Approach, to: ActivityPhase::Act }
                if *id == activity_id
        )).count(), 1);
    }

    #[test]
    fn bottom_forage_completes_beside_the_nudged_ball_without_toy_contact() {
        let mut world = WorldState::new(42, "Forage");
        let mut rng = SeededRandom::new(42);
        world.creature.needs.hunger = 0.9;
        world
            .aquarium
            .toy_states
            .get_mut(&ToyId::Ball)
            .unwrap()
            .position = NormalizedPosition::new(4_785, 8_085);
        let toys_before = world.aquarium.toy_states.clone();
        let mut saw_act = false;
        let mut saw_recover = false;
        let mut completed = false;
        for _ in 0..(20_000 / SIMULATION_TICK_MS) {
            let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::ToyContacted { .. }
                    | GameEvent::ToyObjectResponded { .. }
                    | GameEvent::ToyInteractionResponded { .. }
                    | GameEvent::FoodConsumed(_)
            )));
            if let Some(activity) = world.creature.private_life.active.as_ref() {
                assert_eq!(activity.kind, PrivateLifeKind::BottomForage);
                saw_act |= activity.phase == ActivityPhase::Act;
                saw_recover |= activity.phase == ActivityPhase::Recover;
                if matches!(activity.phase, ActivityPhase::Act | ActivityPhase::Recover) {
                    assert_eq!(world.creature.aquarium.destination, None);
                    assert_eq!(
                        movement_target(&world),
                        None,
                        "foraging must rest clear of every toy"
                    );
                }
            }
            if events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::PrivateLifeCompleted {
                        kind: PrivateLifeKind::BottomForage,
                        ..
                    }
                )
            }) {
                completed = true;
                break;
            }
        }
        assert!(saw_act && saw_recover && completed);
        assert_eq!(world.aquarium.toy_states, toys_before);
        assert_eq!(
            world
                .creature
                .private_life
                .recent
                .iter()
                .filter(|activity| activity.kind == PrivateLifeKind::BottomForage)
                .count(),
            1
        );
    }

    fn begin_private_toy(world: &mut WorldState, toy: ToyId, rng: &mut SeededRandom) {
        world.creature.needs.curiosity = 0.9;
        world.creature.traits.fussiness = 0.1;
        step(world, &[], SIMULATION_TICK_MS, rng);
        let activity = world.creature.private_life.active.as_mut().unwrap();
        activity.kind = PrivateLifeKind::ToyPlay(toy);
        activity.subject = Some(ActivitySubject::Toy(toy));
        activity.recipe = match toy {
            ToyId::Ball => ActivityRecipe::BallNudge,
            ToyId::Bell => ActivityRecipe::BellStrike,
            ToyId::Sock => ActivityRecipe::SockTug,
        };
    }

    fn toy_head_distance(world: &WorldState, toy: ToyId) -> f32 {
        let head = world.creature.aquarium.position;
        let object = world.aquarium.toy_states[&toy].position;
        let x = (head.x - object.x) as f32 * 0.00132;
        let y = (head.y - object.y) as f32 * 0.000455;
        x.hypot(y)
    }

    fn assert_contact_clear_of_neighbors(
        world: &WorldState,
        toy: ToyId,
        position: NormalizedPosition,
    ) {
        let mut neighbors = world.clone();
        neighbors.aquarium.toy_states.remove(&toy);
        neighbors.creature.aquarium.position = position;
        neighbors.creature.aquarium.destination = None;
        neighbors.creature.aquarium.action = None;
        neighbors.creature.private_life.active = None;
        assert_eq!(
            movement_target(&neighbors),
            None,
            "{toy:?} contact {position:?} overlaps a neighbor"
        );
    }

    #[test]
    fn toy_contact_keeps_unobstructed_targets_and_ignores_carried_neighbors() {
        for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
            let mut world = WorldState::new(42, "Unchanged");
            world.aquarium.toy_states.get_mut(&toy).unwrap().position =
                NormalizedPosition::new(5_000, 5_000);
            let mut isolated = world.clone();
            isolated
                .aquarium
                .toy_states
                .retain(|candidate, _| *candidate == toy);
            for position in [
                NormalizedPosition::new(0, 5_000),
                NormalizedPosition::new(10_000, 5_000),
                NormalizedPosition::new(5_000, 0),
                NormalizedPosition::new(5_000, 10_000),
                NormalizedPosition::new(1_000, 1_000),
            ] {
                isolated.creature.aquarium.position = position;
                for carried in [false, true] {
                    for (neighbor, object) in &mut world.aquarium.toy_states {
                        if *neighbor != toy {
                            object.carried = carried;
                            object.position = if carried {
                                NormalizedPosition::new(5_000, 5_000)
                            } else {
                                NormalizedPosition::new(0, 0)
                            };
                        }
                    }
                    world.creature.aquarium.position = position;
                    assert_eq!(
                        approach_position(&world, SemanticDestination::Toy(toy)),
                        approach_position(&isolated, SemanticDestination::Toy(toy))
                    );
                }
            }
        }
    }

    #[test]
    fn neighbor_aware_toy_contacts_are_bounded_deterministic_and_on_the_intended_surface() {
        for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
            for anchor in [
                NormalizedPosition::new(5_000, 5_000),
                NormalizedPosition::new(0, 0),
                NormalizedPosition::new(10_000, 10_000),
            ] {
                let mut world = WorldState::new(42, "Neighbors");
                for (candidate, object) in &mut world.aquarium.toy_states {
                    object.position = if *candidate == toy {
                        anchor
                    } else {
                        NormalizedPosition::new(anchor.x - 1_000, anchor.y).clamped()
                    };
                }
                for start in [
                    NormalizedPosition::new(0, 0),
                    NormalizedPosition::new(10_000, 0),
                    NormalizedPosition::new(0, 10_000),
                    NormalizedPosition::new(10_000, 10_000),
                    anchor,
                ] {
                    world.creature.aquarium.position = start;
                    let target = approach_position(&world, SemanticDestination::Toy(toy));
                    assert_eq!(
                        target,
                        approach_position(&world, SemanticDestination::Toy(toy))
                    );
                    if anchor == NormalizedPosition::new(5_000, 5_000) {
                        assert!(target.is_some(), "the right surface is clear");
                    }
                    if let Some(target) = target {
                        assert_eq!(target, target.clamped());
                        assert_contact_clear_of_neighbors(&world, toy, target);
                        let mut isolated = world.clone();
                        isolated
                            .aquarium
                            .toy_states
                            .retain(|candidate, _| *candidate == toy);
                        isolated.creature.aquarium.position = target;
                        let surface =
                            approach_position(&isolated, SemanticDestination::Toy(toy)).unwrap();
                        assert!(
                            (target.x - surface.x).abs() + (target.y - surface.y).abs() <= 6,
                            "candidate must remain on its own side's {toy:?} surface"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn observed_sock_approach_converges_without_holding_contact_inside_the_bell() {
        let mut contact_positions = Vec::new();
        for private in [false, true] {
            let mut world = WorldState::new(42, "ClearSock");
            let mut rng = SeededRandom::new(42);
            // Reconstruct the authoritative layout at C8 first-five-minutes 194.283 s.
            for (toy, position) in [
                (ToyId::Ball, NormalizedPosition::new(4_753, 8_085)),
                (ToyId::Bell, NormalizedPosition::new(6_650, 8_700)),
                (ToyId::Sock, NormalizedPosition::new(7_794, 10_000)),
            ] {
                world.aquarium.toy_states.get_mut(&toy).unwrap().position = position;
            }
            world.creature.toy_preferences.insert(ToyId::Sock, 0.8);
            if private {
                begin_private_toy(&mut world, ToyId::Sock, &mut rng);
            } else {
                step(&mut world, &[PlayerEvent::Play(ToyId::Sock)], 0, &mut rng);
            }
            let owner = if private {
                world.creature.private_life.active.as_ref().unwrap().id
            } else {
                world
                    .creature
                    .interaction_state
                    .toy_interaction
                    .as_ref()
                    .unwrap()
                    .id
            };
            world.creature.aquarium.position = NormalizedPosition::new(1_740, 8_531);
            let initial_target =
                approach_position(&world, SemanticDestination::Toy(ToyId::Sock)).unwrap();
            assert_ne!(initial_target, NormalizedPosition::new(6_794, 10_000));
            assert_contact_clear_of_neighbors(&world, ToyId::Sock, initial_target);
            let mut act_started = None;
            let mut payoff_at = None;
            let mut responses = 0;
            let mut finished = false;
            for _ in 0..(30_000 / SIMULATION_TICK_MS) {
                let before = world.creature.aquarium.position;
                let travelling = world.creature.aquarium.destination.is_some();
                if payoff_at.is_none() {
                    let target =
                        approach_position(&world, SemanticDestination::Toy(ToyId::Sock)).unwrap();
                    assert_contact_clear_of_neighbors(&world, ToyId::Sock, target);
                }
                let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                if let Some(activity) = world.creature.private_life.active.as_ref()
                    && private
                    && activity.phase == ActivityPhase::Act
                {
                    if act_started.is_none() {
                        contact_positions.push(world.creature.aquarium.position);
                    }
                    act_started.get_or_insert(world.elapsed_ms);
                    assert_contact_clear_of_neighbors(
                        &world,
                        ToyId::Sock,
                        world.creature.aquarium.position,
                    );
                }
                for event in &events {
                    let matched = match event {
                        GameEvent::ToyObjectResponded {
                            toy: ToyId::Sock,
                            activity_id,
                            response: ToyResponse::SockTugged,
                        } => private && *activity_id == owner,
                        GameEvent::ToyInteractionResponded {
                            toy: ToyId::Sock,
                            interaction_id,
                            response: ToyResponse::SockTugged,
                        } => !private && *interaction_id == owner,
                        _ => false,
                    };
                    if matched {
                        if !private {
                            contact_positions.push(before);
                        }
                        responses += 1;
                        payoff_at = Some(world.elapsed_ms);
                        assert!(world.aquarium.toy_states[&ToyId::Sock].carried);
                        assert_contact_clear_of_neighbors(
                            &world,
                            ToyId::Sock,
                            world.creature.aquarium.position,
                        );
                    }
                }
                // Notice is a short in-place pause with gentle drift; the swim itself must
                // converge monotonically.
                if payoff_at.is_none() && travelling {
                    assert!(
                        world.creature.aquarium.position.x >= before.x,
                        "approach must not oscillate horizontally"
                    );
                    assert!(
                        world.creature.aquarium.position.y <= before.y,
                        "approach must converge to the clear upper surface"
                    );
                }
                let completed = if private {
                    events.iter().any(|event| matches!(event, GameEvent::PrivateLifeCompleted { activity_id, .. } if *activity_id == owner))
                } else {
                    payoff_at.is_some()
                        && world.creature.interaction_state.toy_interaction.is_none()
                };
                if completed {
                    if private {
                        // The private Act and Recover phase durations.
                        assert_eq!(payoff_at.unwrap() - act_started.unwrap(), 1_600);
                        assert_eq!(world.elapsed_ms - payoff_at.unwrap(), 900);
                    }
                    assert!(!world.aquarium.toy_states[&ToyId::Sock].carried);
                    finished = true;
                    break;
                }
            }
            assert!(
                finished,
                "{private}: contact must converge in bounded ticks"
            );
            assert_eq!(responses, 1);
            assert_eq!(
                world.aquarium.toy_states[&ToyId::Bell].position,
                NormalizedPosition::new(6_650, 8_700)
            );
        }
        assert_eq!(contact_positions.len(), 2);
        // Private play pauses to notice (with gentle drift) before swimming, so its path
        // starts a few units away. Both must still land on the same surface point within
        // the six-unit contact arrival tolerance.
        let (direct, private) = (contact_positions[0], contact_positions[1]);
        assert!(
            (direct.x - private.x).abs() + (direct.y - private.y).abs() <= 6,
            "direct and private approaches use the same physical surface: {direct:?} {private:?}"
        );
    }

    #[test]
    fn fully_obstructed_toy_approaches_interrupt_once_without_rewards_and_allow_later_input() {
        for private in [false, true] {
            let mut world = WorldState::new(42, "Blocked");
            let mut rng = SeededRandom::new(42);
            world
                .aquarium
                .toy_states
                .retain(|toy, _| *toy != ToyId::Sock);
            for object in world.aquarium.toy_states.values_mut() {
                object.position = NormalizedPosition::new(5_000, 10_000);
            }
            world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
            if private {
                begin_private_toy(&mut world, ToyId::Ball, &mut rng);
            } else {
                step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
            }
            assert_eq!(
                approach_position(&world, SemanticDestination::Toy(ToyId::Ball)),
                None
            );
            let relationship = world.creature.relationship;
            let mut events = Vec::new();
            // Long enough to cover the notice pause before travel begins.
            for _ in 0..(2_000 / SIMULATION_TICK_MS) {
                events.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
            }
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        GameEvent::ToyInteractionInterrupted { .. }
                            | GameEvent::PrivateLifeInterrupted { .. }
                    ))
                    .count(),
                1
            );
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::ToyContacted { .. }
                    | GameEvent::ToyPlayed { .. }
                    | GameEvent::ToyRejected { .. }
                    | GameEvent::ToyObjectResponded { .. }
                    | GameEvent::ToyInteractionResponded { .. }
            )));
            assert_eq!(world.creature.relationship, relationship);
            assert!(world.creature.interaction_state.toy_interaction.is_none());
            assert!(world.creature.private_life.active.is_none());
            assert!(world.creature.aquarium.destination.is_none());
            assert!(world.creature.aquarium.travel_purpose.is_none());
            assert!(
                world
                    .aquarium
                    .toy_states
                    .values()
                    .all(|object| !object.carried && object.last_response == ToyResponse::None)
            );
            assert_eq!(dialogue_handoff(&world).state, DialogueHandoffState::Ready);

            world
                .aquarium
                .toy_states
                .get_mut(&ToyId::Bell)
                .unwrap()
                .position = NormalizedPosition::new(0, 0);
            let accepted = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
            assert!(accepted.iter().any(|event| matches!(
                event,
                GameEvent::ToyPlayAccepted {
                    toy: ToyId::Ball,
                    ..
                }
            )));
            let mut payoffs = 0;
            for _ in 0..(20_000 / SIMULATION_TICK_MS) {
                let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                payoffs += events
                    .iter()
                    .filter(|event| {
                        matches!(
                            event,
                            GameEvent::ToyInteractionResponded {
                                toy: ToyId::Ball,
                                ..
                            }
                        )
                    })
                    .count();
                if payoffs > 0 {
                    break;
                }
            }
            assert_eq!(payoffs, 1, "the later valid play remains usable");
        }
    }

    #[test]
    fn obstructed_relationship_toy_travel_interrupts_its_owner_once_and_allows_later_input() {
        for familiar_place in [true, false] {
            let mut world = WorldState::new(42, "BlockedRelationship");
            let mut rng = SeededRandom::new(42);
            let destination = SemanticDestination::Toy(ToyId::Ball);
            world
                .aquarium
                .toy_states
                .retain(|toy, _| *toy != ToyId::Sock);
            for object in world.aquarium.toy_states.values_mut() {
                object.position = NormalizedPosition::new(5_000, 10_000);
            }
            world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
            let (motif, trigger) = if familiar_place {
                world.creature.idle_life.visit_evidence.push(VisitEvidence {
                    hour_start: 0,
                    destination,
                    visits: 2,
                    last_active_day: 1,
                });
                world.set_routine(Routine {
                    hour_start: 0,
                    destination,
                    strength: 2,
                });
                (
                    RelationshipMotifKey::FamiliarPlace(destination),
                    RelationshipTrigger::RoutineWindow {
                        hour_start: 0,
                        destination,
                    },
                )
            } else {
                world.remember(
                    MemoryKind::PlayedWith { toy: ToyId::Ball },
                    &[Concept::Toy],
                    0.6,
                    0.7,
                );
                // Quiet callbacks require aged memory evidence. The routine-triggered
                // FamiliarPlace case above already has its required visit/routine evidence.
                world.elapsed_ms = relationship::RELATIONSHIP_EVIDENCE_MATURITY_MS;
                (
                    RelationshipMotifKey::SharedToy(ToyId::Ball),
                    RelationshipTrigger::QuietMoment,
                )
            };
            assert_eq!(
                select_relationship_beat(&world, trigger).map(|beat| beat.motif),
                Some(motif),
                "fixture must make {motif:?} eligible through {trigger:?}"
            );
            let started = trigger_relationship_beat(&mut world, trigger);
            assert!(started.iter().any(|event| matches!(event, GameEvent::RelationshipBeatStarted { motif: started_motif, .. } if *started_motif == motif)), "{motif:?} must start through the public trigger: {started:?}");
            assert_eq!(approach_position(&world, destination), None);
            let relationship_before = world.creature.relationship;
            let mut events = Vec::new();
            let mut interrupted_at = None;
            // Long enough to cover the beat's pause before travel begins, then one more
            // second (shorter than any idle bout) to prove the interruption happens once.
            for tick_index in 0..(6_000 / SIMULATION_TICK_MS) {
                let tick = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                let interrupted_now = tick.iter().any(|event| matches!(event, GameEvent::RelationshipBeatInterrupted(interrupted_motif) if *interrupted_motif == motif));
                if interrupted_now {
                    interrupted_at.get_or_insert(tick_index);
                }
                if interrupted_at.is_some() {
                    assert!(world.creature.relationship_expression.active.is_none());
                    assert!(world.creature.aquarium.destination.is_none());
                    assert!(world.creature.aquarium.travel_purpose.is_none());
                    assert_eq!(dialogue_handoff(&world).state, DialogueHandoffState::Ready);
                }
                events.extend(tick);
                if interrupted_at.is_some_and(|at| tick_index >= at + 1_000 / SIMULATION_TICK_MS) {
                    break;
                }
            }
            assert_eq!(events.iter().filter(|event| matches!(event, GameEvent::RelationshipBeatInterrupted(interrupted_motif) if *interrupted_motif == motif)).count(), 1);
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::ToyContacted { .. }
                    | GameEvent::ToyPlayed { .. }
                    | GameEvent::ToyRejected { .. }
                    | GameEvent::ToyObjectResponded { .. }
                    | GameEvent::ToyInteractionResponded { .. }
                    | GameEvent::RelationshipBeatCompleted(_)
                    | GameEvent::RelationshipBeatPhaseChanged {
                        to: RelationshipBeatPhase::Act,
                        ..
                    }
            )));
            assert_eq!(world.creature.relationship, relationship_before);
            assert!(world.creature.private_life.active.is_none());
            assert!(world.creature.interaction_state.toy_interaction.is_none());
            assert!(
                world
                    .aquarium
                    .toy_states
                    .values()
                    .all(|object| !object.carried && object.last_response == ToyResponse::None)
            );

            world
                .aquarium
                .toy_states
                .get_mut(&ToyId::Bell)
                .unwrap()
                .position = NormalizedPosition::new(0, 0);
            let accepted = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
            assert!(accepted.iter().any(|event| matches!(
                event,
                GameEvent::ToyPlayAccepted {
                    toy: ToyId::Ball,
                    ..
                }
            )));
            let mut payoffs = 0;
            for _ in 0..(20_000 / SIMULATION_TICK_MS) {
                let tick = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                payoffs += tick
                    .iter()
                    .filter(|event| {
                        matches!(
                            event,
                            GameEvent::ToyInteractionResponded {
                                toy: ToyId::Ball,
                                ..
                            }
                        )
                    })
                    .count();
                if payoffs > 0 {
                    break;
                }
            }
            assert_eq!(
                payoffs, 1,
                "an obstructed relationship must not suppress later play"
            );
        }
    }

    #[test]
    fn toy_surface_contact_handles_all_approach_sides_and_tank_edges() {
        for (anchor, start) in [
            ((5_000, 5_000), (1_000, 5_000)),
            ((5_000, 5_000), (9_000, 5_000)),
            ((5_000, 5_000), (5_000, 0)),
            ((5_000, 5_000), (5_000, 10_000)),
            ((5_000, 5_000), (5_000, 5_000)),
            ((0, 0), (0, 0)),
            ((10_000, 10_000), (10_000, 10_000)),
        ] {
            let mut world = WorldState::new(201, "Surface");
            let mut rng = SeededRandom::new(201);
            let anchor = NormalizedPosition::new(anchor.0, anchor.1);
            world
                .aquarium
                .toy_states
                .retain(|toy, _| *toy == ToyId::Ball);
            world
                .aquarium
                .toy_states
                .get_mut(&ToyId::Ball)
                .unwrap()
                .position = anchor;
            world.creature.aquarium.position = NormalizedPosition::new(start.0, start.1);
            world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
            let receipt = step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
            assert!(
                !receipt
                    .iter()
                    .any(|event| matches!(event, GameEvent::ToyContacted { .. }))
            );
            let mut contacts = 0;
            // The closest vertical ball contact is 2_200 * 0.000455 = 1.001
            // world units. Actual rendered surface bounds are checked in beastie-game.
            let minimum_clearance = 0.98;
            let started_clear = toy_head_distance(&world, ToyId::Ball) > minimum_clearance;
            for tick in 0..20 {
                let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                assert_eq!(world.aquarium.toy_states[&ToyId::Ball].position, anchor);
                if started_clear {
                    assert!(toy_head_distance(&world, ToyId::Ball) > minimum_clearance);
                }
                contacts += events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::ToyContacted { .. }))
                    .count();
                if contacts > 0 {
                    assert!(toy_head_distance(&world, ToyId::Ball) > minimum_clearance);
                    break;
                }
                assert!(tick < 19, "surface contact must finish from {start:?}");
            }
            assert_eq!(contacts, 1);
            assert_eq!(
                world.creature.aquarium.position,
                world.creature.aquarium.position.clamped()
            );
        }
    }

    #[test]
    fn private_toy_contact_and_quiet_rest_keep_separation_and_nudge_away() {
        for toy in [ToyId::Ball, ToyId::Bell] {
            for side in [-1, 1] {
                let mut world = WorldState::new(212, "Space");
                let mut rng = SeededRandom::new(212);
                world
                    .aquarium
                    .toy_states
                    .retain(|candidate, _| *candidate == toy);
                world.aquarium.toy_states.get_mut(&toy).unwrap().position =
                    NormalizedPosition::new(5_000, 6_500);
                world.creature.aquarium.position =
                    NormalizedPosition::new(5_000 + side * 3_000, 6_500);
                begin_private_toy(&mut world, toy, &mut rng);
                let mut responses = 0;
                let mut completed = false;
                for _ in 0..50 {
                    let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                    assert!(
                        toy_head_distance(&world, toy) > 1.1,
                        "{toy:?} must not occupy the head"
                    );
                    if events
                        .iter()
                        .any(|event| matches!(event, GameEvent::ToyObjectResponded { .. }))
                    {
                        responses += 1;
                        if toy == ToyId::Ball {
                            assert_eq!(world.aquarium.toy_states[&toy].velocity.x.signum(), -side);
                        }
                    }
                    if events
                        .iter()
                        .any(|event| matches!(event, GameEvent::PrivateLifeCompleted { .. }))
                    {
                        completed = true;
                        world.creature.idle_life.settled_until_ms = world.elapsed_ms + 60_000;
                    }
                }
                assert!(completed);
                assert_eq!(responses, 1);
                assert!(!world.aquarium.toy_states[&toy].carried);
            }
        }
    }

    #[test]
    fn sock_release_starts_at_held_anchor_and_falls_with_exact_saved_continuation() {
        let mut world = WorldState::new(212, "Drop");
        let mut rng = SeededRandom::new(212);
        world
            .aquarium
            .toy_states
            .retain(|toy, _| *toy == ToyId::Sock);
        world
            .aquarium
            .toy_states
            .get_mut(&ToyId::Sock)
            .unwrap()
            .position = NormalizedPosition::new(5_000, 9_000);
        world.creature.aquarium.position = NormalizedPosition::new(5_000, 3_000);
        begin_private_toy(&mut world, ToyId::Sock, &mut rng);
        let mut release = None;
        for _ in 0..(30_000 / SIMULATION_TICK_MS) {
            let before = world.aquarium.toy_states[&ToyId::Sock].clone();
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            let sock = &world.aquarium.toy_states[&ToyId::Sock];
            if before.carried && !sock.carried {
                assert_eq!(
                    sock.position, before.position,
                    "release cannot teleport the held sock"
                );
                assert_eq!(
                    sock.velocity,
                    NormalizedVelocity {
                        x: 0,
                        y: SOCK_RELEASE_SPEED
                    }
                );
                release = Some(sock.position);
                break;
            }
        }
        let release = release.expect("the completed tug releases its sock");
        let encoded = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded).unwrap().resume();
        let mut individual = Vec::new();
        for _ in 0..8 {
            individual.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
        }
        let batched = step(&mut resumed, &[], SIMULATION_TICK_MS * 8, &mut resumed_rng);
        assert_eq!(batched, individual);
        assert_eq!(resumed, world);
        assert!(!world.aquarium.toy_states[&ToyId::Sock].carried);
        assert!(world.aquarium.toy_states[&ToyId::Sock].position.y > release.y);
        assert!(toy_head_distance(&world, ToyId::Sock) > 0.9);
    }

    #[test]
    fn accepted_toy_mutates_only_once_at_physical_contact() {
        let mut world = WorldState::new(201, "Contact");
        let mut rng = SeededRandom::new(201);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Ball)).unwrap();
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
            GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
                | GameEvent::ToyInteractionResponded { .. }
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
                | GameEvent::ToyInteractionResponded { interaction_id: candidate, .. }
                if *candidate == interaction_id
        )));
    }

    #[test]
    fn direct_toy_contact_commits_each_physical_response_once_with_exact_identity() {
        for (toy, start) in [
            (ToyId::Ball, (1_000, 5_000)),
            (ToyId::Ball, (9_000, 5_000)),
            (ToyId::Ball, (5_000, 1_000)),
            (ToyId::Ball, (5_000, 9_000)),
            (ToyId::Bell, (1_000, 5_000)),
            (ToyId::Sock, (1_000, 5_000)),
        ] {
            let mut world = WorldState::new(201, "Contact");
            let mut rng = SeededRandom::new(201);
            let anchor = NormalizedPosition::new(5_000, 5_000);
            world
                .aquarium
                .toy_states
                .retain(|candidate, _| *candidate == toy);
            let object = world.aquarium.toy_states.get_mut(&toy).unwrap();
            object.position = anchor;
            // A prior private activity may have the same numeric ID. It must
            // neither suppress this direct contact nor be reported as its owner.
            object.last_contact_activity = std::num::NonZeroU64::new(1);
            let before = object.clone();
            world.creature.aquarium.position = NormalizedPosition::new(start.0, start.1);
            world.creature.toy_preferences.insert(toy, 0.8);
            world.creature.idle_life.settled_until_ms = 60_000;
            let receipt = step(&mut world, &[PlayerEvent::Play(toy)], 0, &mut rng);
            let interaction_id = receipt
                .iter()
                .find_map(|event| match event {
                    GameEvent::ToyPlayAccepted { interaction_id, .. } => Some(*interaction_id),
                    _ => None,
                })
                .unwrap();
            // One round isolates the per-contact exactly-once property; multi-round sessions
            // have their own test.
            world
                .creature
                .interaction_state
                .toy_interaction
                .as_mut()
                .unwrap()
                .rounds_left = 0;
            assert_eq!(world.aquarium.toy_states[&toy], before);
            assert!(!receipt.iter().any(|event| matches!(
                event,
                GameEvent::ToyInteractionResponded { .. } | GameEvent::ToyObjectResponded { .. }
            )));
            let response = match toy {
                ToyId::Ball => ToyResponse::BallNudged,
                ToyId::Bell => ToyResponse::BellStruck,
                ToyId::Sock => ToyResponse::SockTugged,
            };
            let expected = GameEvent::ToyInteractionResponded {
                toy,
                interaction_id,
                response,
            };
            let mut contacted = false;
            for _ in 0..20 {
                let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                if events.contains(&GameEvent::ToyContacted {
                    toy,
                    interaction_id,
                    origin: ToyOrigin::Player,
                }) {
                    assert_eq!(events.iter().filter(|event| **event == expected).count(), 1);
                    assert!(
                        !events
                            .iter()
                            .any(|event| matches!(event, GameEvent::ToyObjectResponded { .. }))
                    );
                    contacted = true;
                    break;
                }
                assert!(!events.contains(&expected));
                assert_eq!(
                    world.aquarium.toy_states[&toy], before,
                    "{toy:?} changed before contact"
                );
            }
            assert!(contacted, "{toy:?} must reach contact");
            let object = &world.aquarium.toy_states[&toy];
            assert_eq!(object.last_contact_activity, Some(interaction_id));
            assert_eq!(object.last_response, response);
            match toy {
                ToyId::Ball => {
                    assert_eq!(
                        object.position, anchor,
                        "new impulse begins on the following tick"
                    );
                    assert_eq!(object.velocity.x.signum(), (anchor.x - start.0).signum());
                    assert_eq!(object.velocity.y.signum(), (anchor.y - start.1).signum());
                    assert_ne!(object.velocity, NormalizedVelocity::default());
                }
                ToyId::Bell => {
                    assert_eq!(object.position, anchor);
                    assert_eq!(object.velocity, NormalizedVelocity::default());
                }
                ToyId::Sock => {
                    assert!(object.carried);
                    assert_eq!(
                        object.position,
                        held_toy_position(world.creature.aquarium.position)
                    );
                }
            }
            let encoded = SaveGame::capture(&world, &rng).to_json().unwrap();
            let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded).unwrap().resume();
            assert_eq!(resumed, world);
            // Recovery holds up to 2.6 s (the towed sock); step past it in both forms.
            let mut individual = Vec::new();
            for _ in 0..30 {
                individual.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
            }
            let batched = step(&mut resumed, &[], SIMULATION_TICK_MS * 30, &mut resumed_rng);
            assert_eq!(batched, individual);
            assert_eq!(resumed, world);
            assert!(
                !batched.contains(&expected),
                "saved contact cannot respond twice"
            );
            assert!(!world.aquarium.toy_states[&toy].carried);
        }
    }

    #[test]
    fn direct_sock_recovery_survives_talk_and_releases_after_save_without_a_jump() {
        let mut world = WorldState::new(201, "Hold");
        let mut rng = SeededRandom::new(201);
        world.creature.toy_preferences.insert(ToyId::Sock, 0.8);
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Sock)).unwrap();
        step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Sock)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        let held = world.aquarium.toy_states[&ToyId::Sock].clone();
        assert!(held.carried);
        let owner = world.creature.interaction_state.toy_interaction.clone();
        let talk = step(&mut world, &[PlayerEvent::Talk], 0, &mut rng);
        assert_eq!(world.aquarium.toy_states[&ToyId::Sock], held);
        assert_eq!(world.creature.interaction_state.toy_interaction, owner);
        assert!(!talk.iter().any(|event| matches!(
            event,
            GameEvent::ToyInteractionInterrupted { .. } | GameEvent::ToyInteractionResponded { .. }
        )));
        let encoded = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&encoded).unwrap().resume();
        // The hold lasts a readable moment, then releases identically after the save.
        let mut next = Vec::new();
        let mut held_ticks = 0;
        let mut last_held = held.position;
        while world.aquarium.toy_states[&ToyId::Sock].carried {
            assert!(held_ticks < 40, "the sock hold must end");
            held_ticks += 1;
            last_held = world.aquarium.toy_states[&ToyId::Sock].position;
            next = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            assert_eq!(
                step(&mut resumed, &[], SIMULATION_TICK_MS, &mut resumed_rng),
                next
            );
            assert_eq!(resumed, world);
        }
        assert!(held_ticks >= 10, "the hold is visible, not a single tick");
        let released = &world.aquarium.toy_states[&ToyId::Sock];
        assert!(!released.carried);
        // Release starts exactly where the sock was last held, without a jump.
        assert_eq!(released.position, last_held);
        assert_eq!(
            released.velocity,
            NormalizedVelocity {
                x: 0,
                y: SOCK_RELEASE_SPEED
            }
        );
        assert!(
            !next
                .iter()
                .any(|event| matches!(event, GameEvent::ToyInteractionResponded { .. }))
        );
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        // Velocity is per second; one tick moves the matching fraction of it.
        let fall = SOCK_RELEASE_SPEED * SIMULATION_TICK_MS as i32 / 1_000;
        assert_eq!(
            world.aquarium.toy_states[&ToyId::Sock].position.y,
            (last_held.y + fall).min(10_000)
        );
    }

    #[test]
    fn direct_sock_replacement_and_offline_recovery_never_strand_or_replay_the_hold() {
        let mut world = WorldState::new(201, "Release");
        let mut rng = SeededRandom::new(201);
        world.creature.toy_preferences.insert(ToyId::Sock, 0.8);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Sock)).unwrap();
        step(
            &mut world,
            &[PlayerEvent::Play(ToyId::Sock)],
            SIMULATION_TICK_MS,
            &mut rng,
        );
        assert!(world.aquarium.toy_states[&ToyId::Sock].carried);
        let held = world.aquarium.toy_states[&ToyId::Sock].clone();
        let mut offline = world.clone();
        let mut offline_rng = rng;
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let released = &world.aquarium.toy_states[&ToyId::Sock];
        assert!(!released.carried);
        assert_eq!(released.position, held.position);
        assert_eq!(released.last_contact_activity, held.last_contact_activity);
        let progress = advance_offline(&mut offline, SIMULATION_TICK_MS, &mut offline_rng);
        assert!(!offline.aquarium.toy_states[&ToyId::Sock].carried);
        assert_eq!(offline.aquarium.toy_states[&ToyId::Sock], *released);
        assert!(!progress.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyInteractionResponded { .. }
                | GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
        )));
        let after = offline.aquarium.toy_states[&ToyId::Sock].clone();
        advance_offline(&mut offline, SIMULATION_TICK_MS, &mut offline_rng);
        assert_eq!(offline.aquarium.toy_states[&ToyId::Sock], after);
    }

    #[test]
    fn rejected_toy_arrival_never_becomes_positive_evidence() {
        let mut world = WorldState::new(202, "Refusal");
        let mut rng = SeededRandom::new(202);
        world.creature.toy_preferences.insert(ToyId::Sock, -1.0);
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Sock)).unwrap();
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
            GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
                | GameEvent::ToyInteractionResponded { .. }
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
            GameEvent::ToyPlayed { interaction_id: candidate, .. }
                | GameEvent::ToyInteractionResponded { interaction_id: candidate, .. }
                if *candidate == interaction_id
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
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Bell)).unwrap();
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
        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Bell)).unwrap();
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
            recovery_until_ms: 0,
            rounds_left: 0,
            contacts: 0,
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
        let interaction_id = world
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .unwrap()
            .id;
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
                | GameEvent::ToyInteractionResponded { .. }
        )));
        assert_eq!(
            resumed.aquarium.toy_states[&ToyId::Ball].last_response,
            ToyResponse::BallNudged
        );
        assert_eq!(
            resumed.aquarium.toy_states[&ToyId::Ball].last_contact_activity,
            Some(interaction_id)
        );
        let responded = resumed.aquarium.toy_states[&ToyId::Ball].clone();
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
        assert_eq!(again.aquarium.toy_states[&ToyId::Ball], responded);
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
    fn offline_pending_toy_finishes_recovery_before_the_player_return_greeting() {
        let mut world = WorldState::new(206, "Welcome");
        let mut rng = SeededRandom::new(206);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        assert_eq!(
            world
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .unwrap()
                .phase,
            ToyInteractionPhase::Approach
        );

        let progress = advance_offline(&mut world, SIMULATION_TICK_MS, &mut rng);
        assert!(world.creature.interaction_state.toy_interaction.is_none());
        assert!(progress.events.iter().any(|event| matches!(
            event,
            GameEvent::RelationshipBeatStarted {
                motif: RelationshipMotifKey::PlayerReturns,
                ..
            }
        )));
        assert_eq!(
            world
                .creature
                .relationship_expression
                .active
                .as_ref()
                .unwrap()
                .motif,
            RelationshipMotifKey::PlayerReturns
        );
        assert!(!progress.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
                | GameEvent::ToyInteractionResponded { .. }
        )));
        let responded = world.aquarium.toy_states[&ToyId::Ball].clone();
        assert_eq!(responded.last_response, ToyResponse::BallNudged);
        advance_offline(&mut world, SIMULATION_TICK_MS, &mut rng);
        assert_eq!(world.aquarium.toy_states[&ToyId::Ball], responded);
        assert_eq!(
            world
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
        let activity = first
            .creature
            .private_life
            .active
            .as_ref()
            .expect("autonomous private-life owner is created with the visit");
        assert!(matches!(activity.kind, PrivateLifeKind::ToyPlay(_)));
        assert_eq!(activity.phase, ActivityPhase::Notice);
        let interaction_id = activity.id;
        assert_eq!(first.creature.aquarium.travel_purpose, None);
        assert!(first_events.contains(&GameEvent::PrivateLifeStarted {
            activity_id: interaction_id,
            kind: activity.kind,
            recipe: activity.recipe,
        }));
        // The notice pause owns the activity without travel; travel begins once it ends.
        for _ in 0..(3_000 / SIMULATION_TICK_MS) {
            step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng);
            if first.creature.aquarium.travel_purpose.is_some() {
                break;
            }
            assert_eq!(
                first
                    .creature
                    .private_life
                    .active
                    .as_ref()
                    .map(|activity| activity.id),
                Some(interaction_id)
            );
        }
        assert_eq!(
            first.creature.aquarium.travel_purpose,
            Some(TravelPurpose::PrivateLife {
                activity_id: interaction_id
            })
        );

        let json = SaveGame::capture(&first, &first_rng).to_json().unwrap();
        let (zero_advance, _) = SaveGame::from_json(&json).unwrap().resume();
        assert_eq!(
            zero_advance
                .creature
                .private_life
                .active
                .as_ref()
                .map(|activity| activity.id),
            Some(interaction_id)
        );

        let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();
        let progress = advance_offline(&mut resumed, SIMULATION_TICK_MS, &mut resumed_rng);
        assert!(!progress.events.iter().any(|event| matches!(
            event,
            GameEvent::ToyPlayAccepted { .. }
                | GameEvent::ToyContacted { .. }
                | GameEvent::ToyPlayed { .. }
                | GameEvent::ToyObjectResponded { .. }
        )));
        assert_eq!(
            resumed
                .aquarium
                .toy_states
                .values()
                .find_map(|toy| toy.last_contact_activity),
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
    fn private_life_seed_42_has_breadth_and_exact_three_minute_replay() {
        let mut first = WorldState::new(42, "Private");
        let mut replay = first.clone();
        let mut first_rng = SeededRandom::new(42);
        let mut replay_rng = first_rng;
        for _ in 0..(180_000 / SIMULATION_TICK_MS) {
            assert_eq!(
                step(&mut first, &[], SIMULATION_TICK_MS, &mut first_rng),
                step(&mut replay, &[], SIMULATION_TICK_MS, &mut replay_rng),
            );
        }
        assert_eq!(first, replay);
        assert_eq!(first_rng, replay_rng);
        let recent = &first.creature.private_life.recent;
        assert!(recent.len() >= 6);
        assert!(
            recent
                .iter()
                .map(|entry| entry.kind)
                .collect::<BTreeSet<_>>()
                .len()
                >= 3
        );
        assert!(recent.windows(2).all(|pair| pair[0].kind != pair[1].kind));
        assert!(
            recent
                .windows(2)
                .all(|pair| pair[0].recipe != pair[1].recipe)
        );
    }

    #[test]
    fn private_life_cross_seed_sequences_are_temperament_shaped() {
        let sequence = |seed| {
            let mut world = WorldState::new(seed, "Shape");
            let mut rng = SeededRandom::new(seed);
            for _ in 0..(120_000 / SIMULATION_TICK_MS) {
                step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            }
            world
                .creature
                .private_life
                .recent
                .iter()
                .map(|entry| entry.kind)
                .collect::<Vec<_>>()
        };
        let first = sequence(7);
        let second = sequence(91);
        assert!(first.len() >= 4 && second.len() >= 4);
        assert_ne!(first, second);
        assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(second.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn autonomous_toy_response_happens_at_contact_once_and_round_trips() {
        let mut world = WorldState::new(212, "Object");
        world.creature.needs.curiosity = 0.9;
        world.creature.traits.fussiness = 0.1;
        let mut rng = SeededRandom::new(212);
        let mut contacts = Vec::new();
        for _ in 0..(16_000 / SIMULATION_TICK_MS) {
            contacts.extend(step(&mut world, &[], SIMULATION_TICK_MS, &mut rng));
        }
        let response = contacts
            .iter()
            .find_map(|event| match event {
                GameEvent::ToyObjectResponded {
                    toy,
                    activity_id,
                    response,
                } => Some((*toy, *activity_id, *response)),
                _ => None,
            })
            .expect("private toy reaches its exact contact boundary");
        assert_eq!(
            contacts
                .iter()
                .filter(|event| matches!(event, GameEvent::ToyObjectResponded { activity_id, .. } if *activity_id == response.1))
                .count(),
            1
        );
        let object = world.aquarium.toy_states.get(&response.0).unwrap();
        assert_eq!(object.last_contact_activity, Some(response.1));
        assert_eq!(object.last_response, response.2);
        let encoded = SaveGame::capture(&world, &rng).to_json().unwrap();
        let (reloaded, _) = SaveGame::from_json(&encoded).unwrap().resume();
        assert_eq!(reloaded.aquarium.toy_states, world.aquarium.toy_states);
    }

    #[test]
    fn new_toy_catalogue_and_mutable_positions_agree() {
        let world = WorldState::new(42, "Belongings");
        for toy in [ToyId::Ball, ToyId::Bell, ToyId::Sock] {
            assert_eq!(
                toy_position(&world, toy),
                world.aquarium.toy_states[&toy].position
            );
        }
    }

    #[test]
    fn moving_ball_and_carried_sock_save_with_exact_continuation() {
        for (toy, recipe, response) in [
            (
                ToyId::Ball,
                ActivityRecipe::BallNudge,
                ToyResponse::BallNudged,
            ),
            (
                ToyId::Sock,
                ActivityRecipe::SockTug,
                ToyResponse::SockTugged,
            ),
        ] {
            let mut world = WorldState::new(212, "KeptBelongings");
            let mut rng = SeededRandom::new(212);
            // A saved habitat can differ from today's defaults. Preserve its catalogue
            // anchor as well as the mutable response position and velocity.
            let saved_position = NormalizedPosition::new(4137, 8123);
            for object in world.aquarium.objects.values_mut() {
                if let WorldObject::Toy {
                    toy: candidate,
                    position,
                } = object
                    && *candidate == toy
                {
                    *position = saved_position;
                }
            }
            world.aquarium.toy_states.get_mut(&toy).unwrap().position = saved_position;
            world.creature.needs.curiosity = 0.9;
            world.creature.traits.fussiness = 0.1;
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            // Pin a valid noticed activity, then let normal approach/contact rules create
            // the moving or carried state rather than fabricating its payoff receipt.
            let activity = world.creature.private_life.active.as_mut().unwrap();
            activity.kind = PrivateLifeKind::ToyPlay(toy);
            activity.recipe = recipe;
            activity.subject = Some(ActivitySubject::Toy(toy));
            let activity_id = activity.id;
            let mut reached_contact = false;
            for _ in 0..(30_000 / SIMULATION_TICK_MS) {
                let events = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                if events.contains(&GameEvent::ToyObjectResponded {
                    toy,
                    activity_id,
                    response,
                }) {
                    reached_contact = true;
                    break;
                }
            }
            assert!(reached_contact, "{toy:?} must reach its real payoff");
            let object = &world.aquarium.toy_states[&toy];
            match toy {
                ToyId::Ball => assert_ne!(object.velocity, NormalizedVelocity::default()),
                ToyId::Sock => assert!(object.carried),
                ToyId::Bell => unreachable!(),
            }
            assert_eq!(object.last_contact_activity, Some(activity_id));
            let json = SaveGame::capture(&world, &rng).to_json().unwrap();
            let (mut resumed, mut resumed_rng) = SaveGame::from_json(&json).unwrap().resume();
            assert_eq!(resumed, world);
            assert_eq!(toy_position(&resumed, toy), saved_position);

            // Water drag needs several seconds to stop a nudged ball completely.
            for _ in 0..(10_000 / SIMULATION_TICK_MS) {
                let expected = step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
                let actual = step(&mut resumed, &[], SIMULATION_TICK_MS, &mut resumed_rng);
                assert_eq!(actual, expected);
                assert_eq!(resumed, world);
                assert!(
                    !actual.iter().any(|event| matches!(event,
                        GameEvent::ToyObjectResponded { activity_id: id, .. } if *id == activity_id
                    )),
                    "resuming must not repeat a saved payoff"
                );
            }
            assert!(!resumed.aquarium.toy_states[&toy].carried);
            if toy == ToyId::Ball {
                assert_eq!(
                    resumed.aquarium.toy_states[&toy].velocity,
                    NormalizedVelocity::default()
                );
            }
        }
    }

    #[test]
    fn dialogue_handoff_binds_exact_owner_and_safe_boundaries() {
        let mut world = WorldState::new(212, "Boundary");
        world.creature.needs.curiosity = 0.9;
        world.creature.traits.fussiness = 0.1;
        let mut rng = SeededRandom::new(212);
        step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
        let private_id = world.creature.private_life.active.as_ref().unwrap().id;
        assert_eq!(
            dialogue_handoff(&world),
            DialogueHandoff {
                owner: Some(DialogueActionOwner::PrivateLife(private_id)),
                state: DialogueHandoffState::WaitingForContact,
            }
        );
        for _ in 0..(12_000 / SIMULATION_TICK_MS) {
            step(&mut world, &[], SIMULATION_TICK_MS, &mut rng);
            if dialogue_handoff(&world).state == DialogueHandoffState::SafeBoundary {
                break;
            }
        }
        assert_eq!(
            dialogue_handoff(&world).state,
            DialogueHandoffState::SafeBoundary
        );

        let mut direct = WorldState::new(8, "Food");
        let mut direct_rng = SeededRandom::new(8);
        step(
            &mut direct,
            &[PlayerEvent::DropFood {
                food: FoodId::Pellet,
                position: NormalizedPosition::new(5_000, 4_500),
            }],
            0,
            &mut direct_rng,
        );
        assert!(matches!(
            dialogue_handoff(&direct),
            DialogueHandoff {
                owner: Some(DialogueActionOwner::Food(_)),
                state: DialogueHandoffState::WaitingForContact,
            }
        ));
    }

    #[test]
    fn v6_save_migrates_private_life_without_replacing_direct_toy_owner() {
        let mut world = WorldState::new(303, "Migrate");
        let mut rng = SeededRandom::new(303);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        let expected = world
            .creature
            .interaction_state
            .toy_interaction
            .as_ref()
            .unwrap()
            .id;
        let mut value = serde_json::to_value(SaveGame::capture(&world, &rng)).unwrap();
        value["save_version"] = serde_json::Value::from(6);
        value["world"]["save_version"] = serde_json::Value::from(6);
        value["world"]["creature"]
            .as_object_mut()
            .unwrap()
            .remove("private_life");
        value["world"]["creature"]["relationship_expression"]
            .as_object_mut()
            .unwrap()
            .remove("performance_ledger");
        value["world"]["aquarium"]
            .as_object_mut()
            .unwrap()
            .remove("toy_states");
        let migrated = SaveGame::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(migrated.save_version, SAVE_VERSION);
        assert!(migrated.world.creature.private_life.active.is_none());
        assert_eq!(
            migrated
                .world
                .creature
                .relationship_expression
                .schema_version,
            RELATIONSHIP_EXPRESSION_SCHEMA_VERSION
        );
        assert!(
            migrated
                .world
                .creature
                .relationship_expression
                .performance_ledger
                .is_empty()
        );
        assert_eq!(
            migrated
                .world
                .creature
                .interaction_state
                .toy_interaction
                .as_ref()
                .map(|interaction| interaction.id),
            Some(expected)
        );
        assert_eq!(migrated.world.aquarium.toy_states.len(), 3);
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

        world.creature.aquarium.position =
            approach_position(&world, SemanticDestination::Toy(ToyId::Ball)).unwrap();
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
