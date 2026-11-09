use engine::game::layers::evaluate_layers;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::parser::oracle::parse_oracle_text;
use engine::types::events::GameEvent;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::TargetRef;

const MS_MARVEL: &str = "Reach, vigilance\nYou have no maximum hand size.\nEmbiggen Fist — \
Whenever you cast a spell that targets a creature you control, draw a card. Until end of turn, Ms. \
Marvel gains \"Ms. Marvel's base power is equal to the number of cards in your hand.\"";

fn advance_to_next_turn(runner: &mut GameRunner) {
    runner.advance_to_phase(Phase::PostCombatMain);
    if matches!(
        runner.state().waiting_for,
        WaitingFor::DeclareAttackers { .. }
    ) {
        runner
            .declare_attackers(&[])
            .expect("an empty attack declaration advances the turn");
        runner.advance_to_phase(Phase::PostCombatMain);
    }
    runner.advance_to_phase(Phase::PreCombatMain);
}

#[test]
fn ms_marvel_granted_base_power_static_reaches_the_granter() {
    let parsed = parse_oracle_text(
        MS_MARVEL,
        "Ms. Marvel, Kamala Khan",
        &["Reach".to_string(), "Vigilance".to_string()],
        &["Creature".to_string()],
        &[],
    );
    let json = serde_json::to_string(&parsed).expect("ParsedAbilities serializes");
    assert_eq!(parsed.triggers.len(), 1, "{parsed:#?}");
    assert!(
        json.contains("\"type\":\"GrantingObject"),
        "the quoted base-power static must bind its named subject to the granter: {json}"
    );
    assert!(
        !json.contains("granter_reference_unreached")
            && !json.contains("\"type\":\"Unimplemented\""),
        "the full Oracle ability must parse without a residual: {json}"
    );
}

#[test]
fn ms_marvel_draws_and_sets_power_until_the_end_of_turn() {
    const SHOCK: &str = "Shock deals 2 damage to any target.";

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let ms_marvel = scenario
        .add_creature_from_oracle(P0, "Ms. Marvel, Kamala Khan", 1, 4, MS_MARVEL)
        .id();
    let shock = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, SHOCK)
        .id();
    scenario.add_card_to_library_top(P0, "Drawn card");
    scenario.add_card_to_library_top(P1, "Opponent draw");
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![])],
    );
    let mut runner = scenario.build();

    let outcome = runner.cast(shock).target_object(ms_marvel).resolve();
    outcome.assert_hand_drawn(P0, 1);
    runner.state_mut().layers_dirty.mark_full();
    evaluate_layers(runner.state_mut());
    let hand_size = runner.state().players[P0.0 as usize].hand.len() as i32;
    assert_eq!(
        runner.state().objects[&ms_marvel].power,
        Some(hand_size),
        "the granted base-power static uses the controller's current hand size"
    );

    // CR 514.2: "until end of turn" effects end during cleanup.
    advance_to_next_turn(&mut runner);
    runner.state_mut().layers_dirty.mark_full();
    evaluate_layers(runner.state_mut());
    assert_eq!(
        runner.state().objects[&ms_marvel].power,
        Some(1),
        "the temporary base-power change expires during the turn transition"
    );
}

#[test]
fn ms_marvel_does_not_trigger_for_a_spell_targeting_an_opponent_creature() {
    const SHOCK: &str = "Shock deals 2 damage to any target.";

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_creature_from_oracle(P0, "Ms. Marvel, Kamala Khan", 1, 4, MS_MARVEL);
    let opponent_creature = scenario.add_creature(P1, "Opponent creature", 2, 4).id();
    let shock = scenario
        .add_spell_to_hand_from_oracle(P0, "Shock", true, SHOCK)
        .id();
    scenario.add_card_to_library_top(P0, "Unchanged draw pile");
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Red, ObjectId(0), false, vec![])],
    );
    let mut runner = scenario.build();

    let outcome = runner
        .cast(shock)
        .target_object(opponent_creature)
        .resolve();
    assert!(
        outcome.events().iter().any(|event| {
            matches!(
                event,
                GameEvent::BecomesTarget {
                    target: TargetRef::Object(target),
                    source_id,
                    ..
                } if *target == opponent_creature && *source_id == shock
            )
        }),
        "reach-guard: Shock was cast targeting the opponent's creature"
    );
    outcome.assert_hand_drawn(P0, 0);
}
