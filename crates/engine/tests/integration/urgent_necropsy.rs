use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::actions::GameAction;
use engine::types::game_state::{CastPaymentMode, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const URGENT_NECROPSY: &str = "As an additional cost to cast this spell, collect evidence X, where X is the total mana value of the permanents this spell targets.\nDestroy up to one target artifact, up to one target creature, up to one target enchantment, and up to one target planeswalker.";

#[derive(Clone, Copy)]
enum TargetKind {
    Artifact,
    Creature,
    Enchantment,
    Planeswalker,
}

#[derive(Clone, Copy)]
struct Targets {
    artifact: ObjectId,
    creature: ObjectId,
    enchantment: ObjectId,
    planeswalker: ObjectId,
}

fn fixture(evidence_mana_value: u32) -> (GameScenario, ObjectId, Targets, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let artifact = scenario
        .add_artifact_from_oracle(P0, "Target Artifact", "")
        .with_mana_cost(ManaCost::generic(2))
        .id();
    let creature = scenario
        .add_creature(P0, "Target Creature", 2, 2)
        .with_mana_cost(ManaCost::generic(3))
        .id();
    let enchantment = scenario
        .add_enchantment_from_oracle(P0, "Target Enchantment", "")
        .with_mana_cost(ManaCost::generic(4))
        .id();
    let planeswalker = scenario
        .add_planeswalker_from_oracle(P0, "Target Planeswalker", "Test", 4, "")
        .with_mana_cost(ManaCost::generic(1))
        .id();
    let evidence = scenario
        .add_creature_to_graveyard(P0, "Evidence", 1, 1)
        .with_mana_cost(ManaCost::generic(evidence_mana_value))
        .id();
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Urgent Necropsy", true, URGENT_NECROPSY)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    (
        scenario,
        spell,
        Targets {
            artifact,
            creature,
            enchantment,
            planeswalker,
        },
        evidence,
    )
}

fn required_evidence_for(target_kinds: &[TargetKind]) -> u32 {
    let (scenario, spell, available, _) = fixture(8);
    let targets = target_kinds
        .iter()
        .map(|kind| match kind {
            TargetKind::Artifact => available.artifact,
            TargetKind::Creature => available.creature,
            TargetKind::Enchantment => available.enchantment,
            TargetKind::Planeswalker => available.planeswalker,
        })
        .collect::<Vec<_>>();
    let mut runner = scenario.build();
    assert!(cast_with_targets(&mut runner, spell, &targets));
    let WaitingFor::CollectEvidenceChoice {
        minimum_mana_value, ..
    } = &runner.state().waiting_for
    else {
        panic!(
            "expected target-dependent Collect Evidence prompt, got {:?}",
            runner.state().waiting_for
        );
    };
    *minimum_mana_value
}

fn cast_with_targets(runner: &mut GameRunner, spell: ObjectId, targets: &[ObjectId]) -> bool {
    let card_id = runner.state().objects[&spell].card_id;
    if runner
        .act(GameAction::CastSpell {
            object_id: spell,
            card_id,
            targets: vec![],
            payment_mode: CastPaymentMode::Auto,
        })
        .is_err()
    {
        return false;
    }
    let mut remaining = targets.to_vec();
    for _ in 0..16 {
        match runner.state().waiting_for.clone() {
            WaitingFor::TargetSelection { selection, .. } => {
                let target = remaining.iter().position(|candidate| {
                    selection
                        .current_legal_targets
                        .iter()
                        .any(|legal| matches!(legal, engine::types::ability::TargetRef::Object(id) if id == candidate))
                });
                let target = target.map(|index| {
                    engine::types::ability::TargetRef::Object(remaining.remove(index))
                });
                if runner.act(GameAction::ChooseTarget { target }).is_err() {
                    return false;
                }
            }
            WaitingFor::CollectEvidenceChoice { .. } => return true,
            _ => return false,
        }
    }
    false
}

#[test]
fn urgent_necropsy_requires_the_targets_total_mana_value() {
    let (scenario, spell, targets, evidence) = fixture(5);
    let mut runner = scenario.build();
    assert!(cast_with_targets(
        &mut runner,
        spell,
        &[targets.artifact, targets.creature]
    ));
    let WaitingFor::CollectEvidenceChoice {
        minimum_mana_value,
        cards,
        ..
    } = &runner.state().waiting_for
    else {
        panic!(
            "expected Collect Evidence after targets are chosen, got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(*minimum_mana_value, 5);
    assert!(cards.contains(&evidence));
    runner
        .act(GameAction::SelectCards {
            cards: vec![evidence],
        })
        .expect("exiling evidence with total mana value 5 should be accepted");
    assert_eq!(runner.state().objects[&spell].zone, Zone::Stack);
}

#[test]
fn urgent_necropsy_cannot_be_cast_when_graveyard_cannot_cover_targets() {
    let (scenario, spell, targets, _) = fixture(4);
    let mut runner = scenario.build();
    let result = cast_with_targets(&mut runner, spell, &[targets.artifact, targets.creature]);
    assert!(
        !result,
        "5 mana value of targets must be uncastable with only 4 mana value in the graveyard"
    );
    assert_eq!(
        runner.state().objects[&spell].zone,
        Zone::Hand,
        "an unaffordable target-dependent additional cost must not move the spell to the stack"
    );
}

#[test]
fn urgent_necropsy_requirement_changes_with_its_target_set() {
    assert_eq!(
        required_evidence_for(&[TargetKind::Artifact, TargetKind::Enchantment]),
        6
    );
    assert_eq!(
        required_evidence_for(&[TargetKind::Creature, TargetKind::Planeswalker]),
        4
    );
}
