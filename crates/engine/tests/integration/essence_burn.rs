use engine::game::scenario::{GameScenario, P0, P1};
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaColor, ManaCost, ManaCostShard};
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const ESSENCE_BURN: &str = "Essence Burn deals 5 damage to target black or green creature or planeswalker. If that permanent would die this turn, exile it instead.";

fn add_essence_burn(scenario: &mut GameScenario) -> ObjectId {
    scenario
        .add_spell_to_hand(P0, "Essence Burn", true)
        .with_mana_cost(ManaCost::Cost {
            shards: vec![ManaCostShard::Red],
            generic: 1,
        })
        .from_oracle_text(ESSENCE_BURN)
        .id()
}

#[test]
fn essence_burn_exiles_lethally_damaged_creature_and_planeswalker() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    for _ in 0..4 {
        scenario.add_basic_land(P0, ManaColor::Red);
    }
    let mut black_creature = scenario.add_creature(P1, "Black Creature", 2, 4);
    black_creature.with_color(vec![ManaColor::Black]);
    let black_creature = black_creature.id();
    let mut green_planeswalker =
        scenario.add_planeswalker_from_oracle(P1, "Green Planeswalker", "Gideon", 5, "");
    green_planeswalker.with_color(vec![ManaColor::Green]);
    let green_planeswalker = green_planeswalker.id();
    let creature_spell = add_essence_burn(&mut scenario);
    let planeswalker_spell = add_essence_burn(&mut scenario);
    let mut runner = scenario.build();

    let creature_outcome = runner
        .cast(creature_spell)
        .target_object(black_creature)
        .resolve();
    creature_outcome.assert_zone(&[black_creature], Zone::Exile);

    let planeswalker_outcome = runner
        .cast(planeswalker_spell)
        .target_object(green_planeswalker)
        .resolve();
    planeswalker_outcome.assert_zone(&[green_planeswalker], Zone::Exile);
}

#[test]
fn essence_burn_does_not_target_a_black_noncreature_artifact() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    scenario.add_basic_land(P0, ManaColor::Red);
    scenario.add_basic_land(P0, ManaColor::Red);
    let mut black_artifact = scenario.add_artifact_from_oracle(P1, "Black Artifact", "");
    black_artifact.with_color(vec![ManaColor::Black]);
    let black_artifact = black_artifact.id();
    let spell = add_essence_burn(&mut scenario);
    let mut runner = scenario.build();

    assert!(
        runner
            .cast(spell)
            .target_object(black_artifact)
            .try_resolve()
            .is_err(),
        "Essence Burn's target must be a creature or planeswalker, not any black permanent"
    );

    let mut colorless_scenario = GameScenario::new();
    colorless_scenario.at_phase(Phase::PreCombatMain);
    colorless_scenario.add_basic_land(P0, ManaColor::Red);
    colorless_scenario.add_basic_land(P0, ManaColor::Red);
    let mut colorless_creature = colorless_scenario.add_creature(P1, "Colorless Creature", 2, 4);
    colorless_creature.with_color(vec![]);
    let colorless_creature = colorless_creature.id();
    let spell = add_essence_burn(&mut colorless_scenario);
    let mut colorless_runner = colorless_scenario.build();
    assert!(
        colorless_runner
            .cast(spell)
            .target_object(colorless_creature)
            .try_resolve()
            .is_err(),
        "Essence Burn's target must be black or green"
    );

    let mut positive_scenario = GameScenario::new();
    positive_scenario.at_phase(Phase::PreCombatMain);
    positive_scenario.add_basic_land(P0, ManaColor::Red);
    positive_scenario.add_basic_land(P0, ManaColor::Red);
    let mut black_creature = positive_scenario.add_creature(P1, "Black Creature", 2, 4);
    black_creature.with_color(vec![ManaColor::Black]);
    let black_creature = black_creature.id();
    let spell = add_essence_burn(&mut positive_scenario);
    let mut positive_runner = positive_scenario.build();
    let outcome = positive_runner
        .cast(spell)
        .target_object(black_creature)
        .resolve();
    outcome.assert_zone(&[black_creature], Zone::Exile);
}
