//! What the creature wants right now, derived from authoritative state.
//!
//! Wants are the player's goals. They are never stored or scored: they are a reading of needs,
//! preferences, vocabulary and current activity, so they cannot drift from what the creature is
//! actually doing. Presentation shows them as a thought bubble; once the creature knows a word
//! for what it wants, it asks for it out loud.

use serde::{Deserialize, Serialize};

use crate::{
    ActWord, ActivityPhase, FoodId, Intention, Meaning, PrivateLifeKind, ToyId, WorldState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Want {
    /// Hungry, for a favorite food when it has one.
    Food(Option<FoodId>),
    /// Wants to play with a particular toy.
    Toy(ToyId),
    /// Wants attention: a pet, a word.
    Company,
    /// Sleepy.
    Sleep,
    /// Busy with something it has no word for, and wondering what it is called.
    NameOf(Meaning),
}

impl Want {
    /// The meaning the creature would use to ask for this.
    #[must_use]
    pub const fn meaning(self) -> Meaning {
        match self {
            Self::Food(Some(food)) => Meaning::Food(food),
            Self::Food(None) => Meaning::Act(ActWord::Eat),
            Self::Toy(toy) => Meaning::Toy(toy),
            Self::Company => Meaning::Act(ActWord::Come),
            Self::Sleep => Meaning::Act(ActWord::Sleep),
            Self::NameOf(meaning) => meaning,
        }
    }
}

/// Wants stop being worth teaching once the creature has a working vocabulary.
const CURIOUS_ABOUT_NAMES_UNTIL: usize = 16;

/// The creature's most pressing want, if any.
#[must_use]
pub fn current_want(state: &WorldState) -> Option<Want> {
    let creature = &state.creature;
    if creature.current_intention == Intention::Sleep {
        return None;
    }
    let eating = creature
        .aquarium
        .action
        .as_ref()
        .is_some_and(|action| action.food.is_some());
    let engaged_toy = engaged_toy(state);
    if creature.needs.energy < 0.3 {
        return Some(Want::Sleep);
    }
    if creature.needs.hunger > 0.6 && !eating {
        return Some(Want::Food(favorite_food(state)));
    }
    if creature.lexicon.learned_count() < CURIOUS_ABOUT_NAMES_UNTIL {
        let engaged = engaged_toy.map(Meaning::Toy).or_else(|| {
            // Only while the food is still there to look at.
            let action = creature.aquarium.action.as_ref()?;
            action
                .food_outcome
                .is_none()
                .then_some(action.food?)
                .map(Meaning::Food)
        });
        if let Some(meaning) = engaged
            && creature.lexicon.word_for(meaning).is_none()
        {
            return Some(Want::NameOf(meaning));
        }
    }
    if creature.needs.comfort < 0.4 && !eating {
        return Some(Want::Company);
    }
    if creature.needs.curiosity > 0.6 && engaged_toy.is_none() && !eating {
        return Some(Want::Toy(favorite_toy(state)));
    }
    None
}

/// The toy the creature is playing with right now, if any.
#[must_use]
pub fn engaged_toy(state: &WorldState) -> Option<ToyId> {
    let creature = &state.creature;
    creature
        .interaction_state
        .toy_interaction
        .as_ref()
        .filter(|interaction| interaction.outcome == crate::ToyInteractionOutcome::Accepted)
        .map(|interaction| interaction.toy)
        .or_else(|| {
            creature
                .private_life
                .active
                .as_ref()
                .and_then(|activity| match activity.kind {
                    PrivateLifeKind::ToyPlay(toy)
                        if matches!(
                            activity.phase,
                            ActivityPhase::Approach | ActivityPhase::Act
                        ) =>
                    {
                        Some(toy)
                    }
                    _ => None,
                })
        })
}

fn favorite_food(state: &WorldState) -> Option<FoodId> {
    [FoodId::Berry, FoodId::Mushroom, FoodId::Pellet]
        .into_iter()
        .filter_map(|food| {
            let preference = state.creature.preferences.get(&food).copied()?;
            (preference > 0.25).then_some((food, preference))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(food, _)| food)
}

fn favorite_toy(state: &WorldState) -> ToyId {
    [ToyId::Ball, ToyId::Bell, ToyId::Sock]
        .into_iter()
        .max_by(|left, right| {
            let preference = |toy: &ToyId| {
                state
                    .creature
                    .toy_preferences
                    .get(toy)
                    .copied()
                    .unwrap_or(0.0)
            };
            preference(left).total_cmp(&preference(right))
        })
        .unwrap_or(ToyId::Ball)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerEvent, SeededRandom, step};

    #[test]
    fn hunger_wants_a_known_favorite_food() {
        let mut world = WorldState::new(4, "Mop");
        world.creature.preferences.clear();
        world.creature.needs.hunger = 0.7;
        assert_eq!(current_want(&world), Some(Want::Food(None)));
        world.creature.preferences.insert(FoodId::Berry, 0.6);
        assert_eq!(current_want(&world), Some(Want::Food(Some(FoodId::Berry))));
    }

    #[test]
    fn playing_with_an_unnamed_toy_invites_a_name_until_it_has_one() {
        let mut world = WorldState::new(4, "Mop");
        let mut rng = SeededRandom::new(4);
        world.creature.toy_preferences.insert(ToyId::Ball, 0.8);
        step(&mut world, &[PlayerEvent::Play(ToyId::Ball)], 0, &mut rng);
        assert_eq!(
            current_want(&world),
            Some(Want::NameOf(Meaning::Toy(ToyId::Ball)))
        );
        world
            .creature
            .lexicon
            .hear("ball", &[(Meaning::Toy(ToyId::Ball), 3)], 0);
        world
            .creature
            .lexicon
            .hear("ball", &[(Meaning::Toy(ToyId::Ball), 3)], 1);
        assert_ne!(
            current_want(&world),
            Some(Want::NameOf(Meaning::Toy(ToyId::Ball)))
        );
    }

    #[test]
    fn a_contented_creature_wants_nothing() {
        let mut world = WorldState::new(4, "Mop");
        world.creature.needs.hunger = 0.2;
        world.creature.needs.energy = 0.9;
        world.creature.needs.comfort = 0.8;
        world.creature.needs.curiosity = 0.3;
        assert_eq!(current_want(&world), None);
    }
}
