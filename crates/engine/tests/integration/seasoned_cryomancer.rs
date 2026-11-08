use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;

const ORACLE: &str = "When this creature enters, draw two cards, then discard two cards. When you discard one or more nonland cards this way, tap up to that many target creatures and put a stun counter on each of them.\n\
{3}{U}{U}, Exile this card from your graveyard: Draw two cards.";

fn add_mana(scenario: &mut GameScenario) {
    scenario.with_mana_pool(
        P0,
        vec![ManaUnit::new(ManaType::Blue, ObjectId(9_999), false, vec![]); 3],
    );
}

fn resolve_cryomancer(
    runner: &mut GameRunner,
    source: ObjectId,
    discards: &[ObjectId],
    targets: &[ObjectId],
    expected_target_slots: Option<usize>,
) {
    let mut cast = runner.cast(source).commit();
    let mut saw_discard = false;
    let mut saw_target_prompt = false;

    for _ in 0..32 {
        if saw_discard
            && cast.state().stack.is_empty()
            && matches!(cast.state().waiting_for, WaitingFor::Priority { .. })
        {
            break;
        }

        match cast.state().waiting_for.clone() {
            WaitingFor::Priority { .. } => {
                cast.act(GameAction::PassPriority)
                    .expect("passing priority must advance the ETB ability");
            }
            WaitingFor::DiscardChoice { player, count, .. } => {
                assert_eq!(player, P0);
                assert_eq!(count, 2);
                assert!(!saw_discard, "the ETB ability should discard only once");
                cast.act(GameAction::SelectCards {
                    cards: discards.to_vec(),
                })
                .expect("choosing the two discarded cards must succeed");
                saw_discard = true;
            }
            WaitingFor::TriggerTargetSelection {
                player,
                target_slots,
                selection,
                ..
            } => {
                assert_eq!(player, P0);
                assert_eq!(Some(target_slots.len()), expected_target_slots);
                let slot = selection.current_slot;
                let target = targets
                    .get(slot)
                    .copied()
                    .map(TargetRef::Object)
                    .expect("each requested target must have a slot");
                assert!(
                    target_slots[slot].legal_targets.contains(&target),
                    "the selected creature must be legal for target slot {slot}"
                );
                cast.act(GameAction::ChooseTarget {
                    target: Some(target),
                })
                .expect("choosing the reflexive target must succeed");
                saw_target_prompt = true;
            }
            other => panic!("unexpected state while resolving Seasoned Cryomancer: {other:?}"),
        }
    }

    assert!(
        saw_discard,
        "the ETB ability must reach its discard instruction"
    );
    assert_eq!(saw_target_prompt, expected_target_slots.is_some());
    assert!(
        cast.state().stack.is_empty(),
        "the ETB ability and any reflexive trigger must finish resolving"
    );
    assert!(
        matches!(cast.state().waiting_for, WaitingFor::Priority { .. }),
        "the resolved ETB ability and reflexive trigger must return priority"
    );
}

fn stun_count(runner: &GameRunner, creature: ObjectId) -> u32 {
    runner.state().objects[&creature]
        .counters
        .get(&CounterType::Stun)
        .copied()
        .unwrap_or_default()
}

#[test]
fn seasoned_cryomancer_targets_up_to_the_number_of_nonland_cards_discarded() {
    // Two nonland discards create a reflexive target prompt with two slots.
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let source = scenario
        .add_creature_to_hand_from_oracle(P0, "Seasoned Cryomancer", 2, 3, ORACLE)
        .id();
    let first_nonland = scenario.add_card_to_hand(P0, "Discarded nonland one");
    let second_nonland = scenario.add_card_to_hand(P0, "Discarded nonland two");
    let land = scenario.add_land_to_hand(P0, "Undiscarded land").id();
    scenario.add_card_to_library_top(P0, "Drawn card one");
    scenario.add_card_to_library_top(P0, "Drawn card two");
    let own_creature = scenario.add_creature(P0, "Own target", 2, 2).id();
    let opponent_creature = scenario.add_creature(P1, "Opponent target", 2, 2).id();
    let untouched_creature = scenario.add_creature(P1, "Untouched creature", 2, 2).id();
    add_mana(&mut scenario);
    let mut runner = scenario.build();

    resolve_cryomancer(
        &mut runner,
        source,
        &[first_nonland, second_nonland],
        &[own_creature, opponent_creature],
        Some(2),
    );
    assert_eq!(
        runner.state().objects[&first_nonland].zone,
        engine::types::zones::Zone::Graveyard
    );
    assert_eq!(
        runner.state().objects[&second_nonland].zone,
        engine::types::zones::Zone::Graveyard
    );
    assert_eq!(
        runner.state().objects[&land].zone,
        engine::types::zones::Zone::Hand
    );
    for target in [own_creature, opponent_creature] {
        assert!(runner.state().objects[&target].tapped);
        assert_eq!(stun_count(&runner, target), 1);
    }
    assert!(!runner.state().objects[&untouched_creature].tapped);
    assert_eq!(stun_count(&runner, untouched_creature), 0);

    // A discarded land moves this way, but does not increase the target count.
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let source = scenario
        .add_creature_to_hand_from_oracle(P0, "Seasoned Cryomancer", 2, 3, ORACLE)
        .id();
    let nonland = scenario.add_card_to_hand(P0, "Discarded nonland");
    let land = scenario.add_land_to_hand(P0, "Discarded land").id();
    scenario.add_card_to_library_top(P0, "Drawn card one");
    scenario.add_card_to_library_top(P0, "Drawn card two");
    let target = scenario.add_creature(P0, "Only target", 2, 2).id();
    let untouched_creature = scenario.add_creature(P1, "Untouched creature", 2, 2).id();
    add_mana(&mut scenario);
    let mut runner = scenario.build();

    resolve_cryomancer(&mut runner, source, &[nonland, land], &[target], Some(1));
    assert!(runner.state().objects[&target].tapped);
    assert_eq!(stun_count(&runner, target), 1);
    assert!(!runner.state().objects[&untouched_creature].tapped);
    assert_eq!(stun_count(&runner, untouched_creature), 0);

    // Discarding only lands reaches the positive discard guard but creates no
    // reflexive trigger or target prompt.
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let source = scenario
        .add_creature_to_hand_from_oracle(P0, "Seasoned Cryomancer", 2, 3, ORACLE)
        .id();
    let first_land = scenario.add_land_to_hand(P0, "Discarded land one").id();
    let second_land = scenario.add_land_to_hand(P0, "Discarded land two").id();
    scenario.add_card_to_library_top(P0, "Drawn card one");
    scenario.add_card_to_library_top(P0, "Drawn card two");
    let untouched_creature = scenario.add_creature(P1, "Untouched creature", 2, 2).id();
    add_mana(&mut scenario);
    let mut runner = scenario.build();

    resolve_cryomancer(&mut runner, source, &[first_land, second_land], &[], None);
    assert!(!runner.state().objects[&untouched_creature].tapped);
    assert_eq!(stun_count(&runner, untouched_creature), 0);
}
