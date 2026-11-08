//! Restless Prairie's temporary Llama creature type is a valid CR subtype,
//! even though it does not appear on a printed card type line.

use engine::game::scenario::{GameScenario, P0};
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;

const RESTLESS_PRAIRIE_ORACLE: &str = "This land enters tapped.\n\
{T}: Add {G} or {W}.\n\
{2}{G}{W}: This land becomes a 3/3 green and white Llama creature until end of turn. It's still a land.\n\
Whenever this land attacks, other creatures you control get +1/+1 until end of turn.";

#[test]
fn restless_prairie_becomes_a_3_3_llama_and_remains_a_land() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let prairie = scenario
        .add_land_from_oracle(P0, "Restless Prairie", RESTLESS_PRAIRIE_ORACLE)
        .id();
    scenario.with_mana_pool(
        P0,
        vec![
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Colorless, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::Green, ObjectId(0), false, vec![]),
            ManaUnit::new(ManaType::White, ObjectId(0), false, vec![]),
        ],
    );
    let mut runner = scenario.build();

    let activation = runner.activate(prairie, 1).resolve();
    assert!(matches!(
        activation.final_waiting_for(),
        WaitingFor::Priority { player: P0 }
    ));
    assert!(activation.state().stack.is_empty());

    let prairie = &runner.state().objects[&prairie];
    assert!(prairie.card_types.core_types.contains(&CoreType::Land));
    assert!(prairie.card_types.core_types.contains(&CoreType::Creature));
    assert!(prairie
        .card_types
        .subtypes
        .iter()
        .any(|subtype| subtype == "Llama"));
    assert_eq!((prairie.power, prairie.toughness), (Some(3), Some(3)));
}
