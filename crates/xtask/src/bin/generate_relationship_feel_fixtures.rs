use beastie_core::{
    ACTIVE_DAY_MS, FoodId, GameEvent, Intention, NormalizedPosition, PlayerEvent, SeededRandom,
    SemanticDestination, SteeringMode, ToyId, WorldState, step,
};
use beastie_session::{DialogueHistory, SESSION_SAVE_VERSION, SessionSave};

fn finish_food(world: &mut WorldState, random: &mut SeededRandom, food: FoodId) {
    let position = world.creature.aquarium.position;
    let mut events = step(
        world,
        &[PlayerEvent::DropFood { food, position }],
        0,
        random,
    );
    for _ in 0..90 {
        events.extend(step(world, &[], 1_000, random));
        if events.iter().any(|event| {
            matches!(event, GameEvent::FoodConsumed(found) | GameEvent::FoodRejected(found) if *found == food)
        }) && world.creature.aquarium.action.is_none()
            && world.creature.interaction_state.relationship_moment.is_none()
        {
            return;
        }
    }
    panic!("food action did not finish");
}

fn food_fixture(seed: u64, food: FoodId, preference: f32) -> SessionSave {
    let mut world = WorldState::new(seed, "Mop");
    let mut random = SeededRandom::new(seed);
    world.creature.preferences.insert(food, preference);
    finish_food(&mut world, &mut random, food);
    world.elapsed_ms = ACTIVE_DAY_MS;
    finish_food(&mut world, &mut random, food);
    world.elapsed_ms = ACTIVE_DAY_MS * 2;
    world.creature.current_intention = Intention::Idle;
    world.creature.aquarium.destination = None;
    world.creature.aquarium.steering = SteeringMode::Hover;
    world.creature.relationship_expression = Default::default();
    world.creature.interaction_state.relationship_moment = None;
    SessionSave {
        version: SESSION_SAVE_VERSION,
        saved_at_ms: world.elapsed_ms,
        world,
        random,
        sequence: 0,
        next_request_id: 1,
        dialogue_history: DialogueHistory::default(),
    }
}

fn familiar_fixture(
    seed: u64,
    destination: SemanticDestination,
    position: NormalizedPosition,
) -> SessionSave {
    let mut world = WorldState::new(seed, "Mop");
    let mut random = SeededRandom::new(seed);
    let hour_start = 3_u8;
    let hour_offset = ACTIVE_DAY_MS * u64::from(hour_start) / 24;
    for day in 0..2 {
        world.elapsed_ms = day * ACTIVE_DAY_MS + hour_offset;
        world.creature.aquarium.position = position;
        world.creature.aquarium.destination = Some(destination);
        world.creature.aquarium.steering = SteeringMode::Approach;
        step(&mut world, &[], 1_000, &mut random);
    }
    world.elapsed_ms = ACTIVE_DAY_MS * 2 + hour_offset;
    world.creature.current_intention = Intention::Idle;
    world.creature.aquarium.destination = None;
    world.creature.aquarium.steering = SteeringMode::Hover;
    world.creature.idle_life.settled_until_ms = 0;
    world.creature.relationship_expression = Default::default();
    SessionSave {
        version: SESSION_SAVE_VERSION,
        saved_at_ms: world.elapsed_ms,
        world,
        random,
        sequence: 0,
        next_request_id: 1,
        dialogue_history: DialogueHistory::default(),
    }
}

fn emit(path: &str, save: &SessionSave) {
    let json = serde_json::to_string(save).expect("fixture serializes");
    println!("*** Add File: {path}");
    println!("+{json}");
}

fn main() {
    println!("*** Begin Patch");
    emit(
        "fixtures/saves/feel/trusted-berry.json",
        &food_fixture(4201, FoodId::Berry, 0.8),
    );
    emit(
        "fixtures/saves/feel/mushroom-grudge.json",
        &food_fixture(4202, FoodId::Mushroom, -0.8),
    );
    emit(
        "fixtures/saves/feel/familiar-cave.json",
        &familiar_fixture(
            4203,
            SemanticDestination::Cave,
            NormalizedPosition::new(1_500, 8_500),
        ),
    );
    emit(
        "fixtures/saves/feel/familiar-plant.json",
        &familiar_fixture(
            4204,
            SemanticDestination::Plant,
            NormalizedPosition::new(3_000, 8_800),
        ),
    );
    emit(
        "fixtures/saves/feel/familiar-ball.json",
        &familiar_fixture(
            4205,
            SemanticDestination::Toy(ToyId::Ball),
            NormalizedPosition::new(5_000, 8_900),
        ),
    );
    println!("*** End Patch");
}
