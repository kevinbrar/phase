use engine::ai_support::candidate_actions_broad;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const JACE_MACHINATIONS: &str = "Until end of turn, you may activate loyalty abilities of Jace planeswalkers you control on any player's turn any time you could cast an instant.\n\
Empower Jace 8. (Put eight loyalty counters on a Jace token you control. If you don't control one, first create a blue Jace planeswalker token with \"[−1]: Surveil 1\" and \"[−3]: Draw a card.\")";

const JACE_BELEREN: &str = "[+2]: Each player draws a card.\n\
[−1]: Target player draws a card.\n\
[−10]: Target player mills twenty cards.";

const TEFERI_TIME_RAVALER: &str =
    "Each opponent can cast spells only any time they could cast a sorcery.\n\
[+1]: Until your next turn, you may cast sorcery spells as though they had flash.\n\
[−3]: Return up to one target artifact, creature, or enchantment to its owner's hand. Draw a card.";

fn setup_opponent_turn() -> (GameRunner, ObjectId, ObjectId, ObjectId, ObjectId) {
    let mut scenario = GameScenario::new_n_player(2, 42);
    scenario.at_phase(Phase::PreCombatMain);
    let jace = scenario
        .add_planeswalker_from_oracle(P0, "Jace Beleren", "Jace", 3, JACE_BELEREN)
        .id();
    let teferi = scenario
        .add_planeswalker_from_oracle(P0, "Teferi, Time Raveler", "Teferi", 4, TEFERI_TIME_RAVALER)
        .id();
    let machinations = scenario
        .add_spell_to_hand_from_oracle(P0, "Jace's Machinations", true, JACE_MACHINATIONS)
        .id();
    let second_machinations = scenario
        .add_spell_to_hand_from_oracle(P0, "Jace's Machinations", true, JACE_MACHINATIONS)
        .id();
    scenario.add_card_to_library_top(P0, "P0 library card");
    scenario.add_card_to_library_top(P1, "P1 library card");
    let mut runner = scenario.build();
    let state = runner.state_mut();
    state.active_player = P1;
    state.priority_player = P0;
    state.waiting_for = WaitingFor::Priority { player: P0 };
    (runner, jace, teferi, machinations, second_machinations)
}

fn has_loyalty_action(runner: &GameRunner, source_id: ObjectId, ability_index: usize) -> bool {
    candidate_actions_broad(runner.state())
        .iter()
        .any(|candidate| {
            matches!(
                candidate.action,
                GameAction::ActivateAbility {
                    source_id: candidate_source,
                    ability_index: candidate_index,
                    ..
                } if candidate_source == source_id && candidate_index == ability_index
            )
        })
}

fn empowered_jace_token(runner: &GameRunner) -> ObjectId {
    *runner
        .state()
        .objects
        .iter()
        .find_map(|(id, object)| {
            (object.name == "Jace"
                && object.zone == Zone::Battlefield
                && object.controller == P0
                && object
                    .card_types
                    .core_types
                    .contains(&CoreType::Planeswalker)
                && object
                    .card_types
                    .subtypes
                    .iter()
                    .any(|subtype| subtype == "Jace"))
            .then_some(id)
        })
        .expect("Empower Jace creates a Jace planeswalker token")
}

fn pass_to_p0(runner: &mut GameRunner) {
    if matches!(
        runner.state().waiting_for,
        WaitingFor::Priority { player: P1 }
    ) {
        runner
            .act(GameAction::PassPriority)
            .expect("P1 can pass priority to P0");
    }
    assert!(matches!(
        runner.state().waiting_for,
        WaitingFor::Priority { player: P0 }
    ));
}

#[test]
fn jace_loyalty_is_not_activatable_on_an_opponents_turn_without_permission() {
    let mut scenario = GameScenario::new_n_player(2, 43);
    scenario.at_phase(Phase::PreCombatMain);
    let jace = scenario
        .add_planeswalker_from_oracle(P0, "Jace Beleren", "Jace", 3, JACE_BELEREN)
        .id();
    let machinations = scenario
        .add_spell_to_hand_from_oracle(P0, "Jace's Machinations", true, JACE_MACHINATIONS)
        .id();
    let mut runner = scenario.build();
    assert!(
        has_loyalty_action(&runner, jace, 0),
        "CR 606.3: Jace's +2 is available during P0's main phase with the stack empty"
    );
    let state = runner.state_mut();
    state.active_player = P1;
    state.priority_player = P0;
    state.waiting_for = WaitingFor::Priority { player: P0 };

    assert!(
        !has_loyalty_action(&runner, jace, 0),
        "CR 606.3: without a timing permission, P0 cannot activate Jace's loyalty ability during P1's turn"
    );

    runner.cast(machinations).commit();
    pass_to_p0(&mut runner);
    assert_eq!(runner.state().stack.len(), 1);
    assert!(
        !has_loyalty_action(&runner, jace, 0),
        "CR 304.5: without a timing permission, P0 cannot activate Jace's loyalty ability with a spell on the stack"
    );
}

#[test]
fn jace_machinations_does_not_grant_opponents_jace_timing() {
    let mut scenario = GameScenario::new_n_player(2, 44);
    scenario.at_phase(Phase::PreCombatMain);
    let opponent_jace = scenario
        .add_planeswalker_from_oracle(P1, "Jace Beleren", "Jace", 3, JACE_BELEREN)
        .id();
    let machinations = scenario
        .add_spell_to_hand_from_oracle(P0, "Jace's Machinations", true, JACE_MACHINATIONS)
        .id();
    let mut runner = scenario.build();

    let state = runner.state_mut();
    state.active_player = P1;
    state.priority_player = P1;
    state.waiting_for = WaitingFor::Priority { player: P1 };
    assert!(
        has_loyalty_action(&runner, opponent_jace, 0),
        "the opponent's Jace ability is available on their own turn"
    );

    let state = runner.state_mut();
    state.active_player = P0;
    state.priority_player = P0;
    state.waiting_for = WaitingFor::Priority { player: P0 };
    runner.cast(machinations).resolve();
    runner
        .act(GameAction::PassPriority)
        .expect("P0 can pass priority to P1");
    assert_eq!(runner.state().active_player, P0);
    assert!(matches!(
        runner.state().waiting_for,
        WaitingFor::Priority { player: P1 }
    ));
    assert!(
        !has_loyalty_action(&runner, opponent_jace, 0),
        "CR 606.3: the permission does not apply to an opponent-controlled Jace"
    );
}

#[test]
fn jace_machinations_allows_new_jace_on_opponents_turn_and_preserves_once_per_turn() {
    let (mut runner, jace, teferi, machinations, second_machinations) = setup_opponent_turn();

    runner.cast(machinations).resolve();
    let token = empowered_jace_token(&runner);
    pass_to_p0(&mut runner);

    assert!(has_loyalty_action(&runner, jace, 0));
    assert!(
        has_loyalty_action(&runner, token, 0),
        "CR 611.2c: a Jace token created after the permission begins is still affected"
    );
    assert!(has_loyalty_action(&runner, token, 1));
    assert!(
        !has_loyalty_action(&runner, teferi, 0),
        "Jace's subtype-qualified permission does not apply to Teferi"
    );

    runner.cast(second_machinations).commit();
    pass_to_p0(&mut runner);
    assert!(
        !runner.state().stack.is_empty(),
        "the second Jace's Machinations must remain on the stack while P0 activates Jace"
    );
    assert!(has_loyalty_action(&runner, jace, 0));
    assert!(!has_loyalty_action(&runner, teferi, 0));

    runner.activate(jace, 0).resolve();
    pass_to_p0(&mut runner);
    assert_eq!(runner.state().objects[&jace].loyalty, Some(5));
    assert!(
        !has_loyalty_action(&runner, jace, 1),
        "CR 606.3: instant-speed permission does not bypass the once-per-turn limit for this Jace"
    );
    assert!(
        has_loyalty_action(&runner, token, 0),
        "CR 606.3: a different Jace permanent may still activate its loyalty ability this turn"
    );
}
