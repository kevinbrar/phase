use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaCost, ManaCostShard, ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const ORACLE: &str = "Destroy all artifacts you don't control, then exile the top X cards of your library, where X is the number of artifacts that were put into graveyards from the battlefield this turn. You may put a creature card exiled this way onto the battlefield. It gains haste. Return it to your hand at the beginning of the next end step.";

#[test]
fn anzrags_rampage_exiles_one_card_per_destroyed_artifact() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let enemy_artifact_a = scenario
        .add_artifact_from_oracle(P1, "Enemy Relic A", "")
        .id();
    let enemy_artifact_b = scenario
        .add_artifact_from_oracle(P1, "Enemy Relic B", "")
        .id();
    let friendly_artifact = scenario
        .add_artifact_from_oracle(P0, "Friendly Relic", "")
        .id();
    scenario.with_library_top(P0, &["Library Top A", "Library Top B", "Library Top C"]);
    scenario.with_mana_pool(
        P0,
        [
            ManaType::Red,
            ManaType::Red,
            ManaType::Colorless,
            ManaType::Colorless,
            ManaType::Colorless,
        ]
        .into_iter()
        .map(|mana_type| ManaUnit::new(mana_type, ObjectId(0), false, vec![]))
        .collect(),
    );
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Anzrag's Rampage", false, ORACLE)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Red, ManaCostShard::Red],
            generic: 3,
        })
        .id();

    let mut runner = scenario.build();
    let library = runner.state().players[0].library.clone();
    assert_eq!(library.len(), 3);
    let top = *library.get(0).expect("library top should exist");
    let second = *library.get(1).expect("second library card should exist");
    let third = *library.get(2).expect("third library card should exist");

    let outcome = runner.cast(spell).resolve();
    let state = outcome.state();

    assert_eq!(state.objects[&enemy_artifact_a].zone, Zone::Graveyard);
    assert_eq!(state.objects[&enemy_artifact_b].zone, Zone::Graveyard);
    assert_eq!(state.objects[&friendly_artifact].zone, Zone::Battlefield);
    assert_eq!(state.objects[&top].zone, Zone::Exile);
    assert_eq!(state.objects[&second].zone, Zone::Exile);
    assert_eq!(state.objects[&third].zone, Zone::Library);
    assert_eq!(state.objects[&spell].zone, Zone::Graveyard);
    assert!(state.stack.is_empty());
    assert!(matches!(
        outcome.final_waiting_for(),
        WaitingFor::Priority { player: P0 }
    ));
}
