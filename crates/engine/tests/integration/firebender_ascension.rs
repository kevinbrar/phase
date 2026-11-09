use engine::game::combat::AttackTarget;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::counter::CounterType;
use engine::types::events::GameEvent;
use engine::types::game_state::{GameState, StackEntryKind, WaitingFor};
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaType;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const FIREBENDER_ORACLE: &str = "When this enchantment enters, create a 2/2 red Soldier creature token with firebending 1.\n\
Whenever a creature you control attacking causes a triggered ability of that creature to trigger, put a quest counter on this enchantment. Then if it has four or more quest counters on it, you may copy that ability. You may choose new targets for the copy.";

const DAXOS_ORACLE: &str = "Daxos can't be blocked by creatures with power 3 or greater.\n\
Whenever Daxos deals combat damage to a player, exile the top card of that player's library. You gain life equal to that card's mana value. Until end of turn, you may cast that card and you may spend mana as though it were mana of any color to cast that spell.";

const OPPONENT_ATTACK_ORACLE: &str = "Whenever a creature attacks, you gain 1 life.";

fn firebending_board(
    starting_quest: u32,
    attacker_controller: engine::types::player::PlayerId,
) -> (GameRunner, ObjectId, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let firebender = scenario
        .add_enchantment_from_oracle(P0, "Firebender Ascension", FIREBENDER_ORACLE)
        .id();
    scenario.with_counter(
        firebender,
        CounterType::Generic("quest".to_string()),
        starting_quest,
    );
    let attacker = scenario
        .add_creature(attacker_controller, "Firebending Soldier", 2, 2)
        .from_oracle_text_with_keywords(&["firebending"], "Firebending 1")
        .id();
    let mut runner = scenario.build();
    if attacker_controller != P0 {
        runner.state_mut().active_player = attacker_controller;
        runner.state_mut().priority_player = attacker_controller;
        runner.state_mut().waiting_for = WaitingFor::Priority {
            player: attacker_controller,
        };
    }
    runner.advance_to_combat();
    (runner, firebender, attacker)
}

fn quest_count(state: &GameState, permanent: ObjectId) -> u32 {
    state.objects[&permanent]
        .counters
        .get(&CounterType::Generic("quest".to_string()))
        .copied()
        .unwrap_or(0)
}

fn red_count(state: &GameState, player: usize) -> usize {
    state.players[player]
        .mana_pool
        .units()
        .filter(|unit| unit.color == ManaType::Red)
        .count()
}

fn advance_to_declare_blockers(runner: &mut GameRunner) {
    for _ in 0..32 {
        match runner.state().waiting_for {
            WaitingFor::DeclareBlockers { .. } => return,
            WaitingFor::Priority { .. } => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("passing priority before blockers must succeed");
            }
            ref other => panic!("expected blockers or priority, got {other:?}"),
        }
    }
    panic!("never reached the declare-blockers prompt");
}

fn declare_and_order_firebending(
    runner: &mut GameRunner,
    firebender: ObjectId,
    attacker: ObjectId,
    defender: engine::types::player::PlayerId,
    firebender_resolves_first: bool,
) {
    runner
        .declare_attackers(&[(attacker, AttackTarget::Player(defender))])
        .expect("attacker declaration succeeds");

    let WaitingFor::OrderTriggers { player, triggers } = runner.state().waiting_for.clone() else {
        panic!(
            "Firebending and Firebender Ascension triggers must be ordered together; got {:?}",
            runner.state().waiting_for
        );
    };
    assert_eq!(player, runner.state().active_player);
    assert_eq!(triggers.len(), 2);
    let firebending_index = triggers
        .iter()
        .position(|trigger| trigger.source_id == attacker)
        .expect("the Soldier's Firebending trigger is present");
    let firebender_index = triggers
        .iter()
        .position(|trigger| trigger.source_id == firebender)
        .expect("Firebender Ascension's trigger is present");

    let order = if firebender_resolves_first {
        vec![firebending_index, firebender_index]
    } else {
        vec![firebender_index, firebending_index]
    };
    runner
        .act(GameAction::OrderTriggers { order })
        .expect("Firebender Ascension is placed above the Firebending trigger");

    let original = runner
        .state()
        .stack
        .iter()
        .find(|entry| {
            entry.source_id == attacker
                && matches!(entry.kind, StackEntryKind::TriggeredAbility { .. })
        })
        .expect("the Soldier's Firebending ability is on the stack");
    let observer = runner
        .state()
        .stack
        .iter()
        .find(|entry| {
            entry.source_id == firebender
                && matches!(entry.kind, StackEntryKind::TriggeredAbility { .. })
        })
        .expect("Firebender Ascension's ability is on the stack");
    let Some(GameEvent::AbilityTriggered {
        trigger_instance,
        source,
        ..
    }) = (match &observer.kind {
        StackEntryKind::TriggeredAbility { trigger_event, .. } => trigger_event.as_ref(),
        _ => None,
    })
    else {
        panic!("Firebender Ascension must retain its AbilityTriggered event");
    };
    assert_eq!(*source, attacker);
    assert_eq!(*trigger_instance, original.id);
}

fn pass_until_optional_effect(runner: &mut GameRunner) {
    for _ in 0..8 {
        match runner.state().waiting_for.clone() {
            WaitingFor::OptionalEffectChoice { player, .. } => {
                assert_eq!(player, P0);
                return;
            }
            WaitingFor::Priority { .. } => {
                runner
                    .act(GameAction::PassPriority)
                    .expect("passing priority advances the stack");
            }
            other => panic!("expected an optional copy choice, got {other:?}"),
        }
    }
    panic!(
        "Firebender Ascension did not offer its copy choice: {:?}",
        runner.state().waiting_for
    );
}

#[test]
fn firebending_attack_increments_quest_and_does_not_copy_before_four() {
    let (mut runner, firebender, soldier) = firebending_board(0, P0);
    declare_and_order_firebending(&mut runner, firebender, soldier, P1, true);
    runner.advance_until_stack_empty();

    assert_eq!(quest_count(runner.state(), firebender), 1);
    assert_eq!(red_count(runner.state(), P0.0 as usize), 1);
    assert!(runner.state().stack.is_empty());
    assert!(matches!(
        runner.state().waiting_for,
        WaitingFor::Priority { .. }
    ));
}

#[test]
fn fourth_quest_counter_can_copy_the_attack_trigger() {
    let (mut runner, firebender, soldier) = firebending_board(3, P0);
    declare_and_order_firebending(&mut runner, firebender, soldier, P1, true);
    pass_until_optional_effect(&mut runner);

    runner
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accepting the copy succeeds");
    runner.advance_until_stack_empty();

    assert_eq!(quest_count(runner.state(), firebender), 4);
    assert_eq!(red_count(runner.state(), P0.0 as usize), 2);
}

#[test]
fn declining_the_fourth_quest_counter_copy_keeps_only_original_effect() {
    let (mut runner, firebender, soldier) = firebending_board(3, P0);
    declare_and_order_firebending(&mut runner, firebender, soldier, P1, true);
    pass_until_optional_effect(&mut runner);

    runner
        .act(GameAction::DecideOptionalEffect { accept: false })
        .expect("declining the copy succeeds");
    runner.advance_until_stack_empty();

    assert_eq!(quest_count(runner.state(), firebender), 4);
    assert_eq!(red_count(runner.state(), P0.0 as usize), 1);
}

#[test]
fn copy_of_an_attack_trigger_that_left_the_stack_is_a_noop() {
    let (mut runner, firebender, soldier) = firebending_board(3, P0);
    declare_and_order_firebending(&mut runner, firebender, soldier, P1, false);

    runner.resolve_top();
    assert_eq!(runner.state().stack.len(), 1);
    assert_eq!(red_count(runner.state(), P0.0 as usize), 1);
    pass_until_optional_effect(&mut runner);
    runner
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accepting a copy of the departed trigger is allowed");
    runner.advance_until_stack_empty();

    assert_eq!(quest_count(runner.state(), firebender), 4);
    assert_eq!(red_count(runner.state(), P0.0 as usize), 1);
}

#[test]
fn combat_damage_trigger_of_attacking_creature_does_not_count() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let firebender = scenario
        .add_enchantment_from_oracle(P0, "Firebender Ascension", FIREBENDER_ORACLE)
        .id();
    let daxos = scenario
        .add_creature_from_oracle(P0, "Daxos of Meletis", 2, 2, DAXOS_ORACLE)
        .id();
    let _blocker = scenario.add_creature(P1, "Grizzly Bears", 2, 2);
    let top_card = scenario.add_card_to_library_top(P1, "Daxos Exile Fixture");
    let mut runner = scenario.build();
    runner.advance_to_combat();

    runner
        .declare_attackers(&[(daxos, AttackTarget::Player(P1))])
        .expect("Daxos attacks");
    advance_to_declare_blockers(&mut runner);
    runner
        .declare_blockers(&[])
        .expect("no blockers are declared");
    runner.combat_damage();

    assert_eq!(runner.state().objects[&top_card].zone, Zone::Exile);
    assert_eq!(quest_count(runner.state(), firebender), 0);
}

#[test]
fn opponent_firebending_attacker_does_not_count() {
    let (mut runner, firebender, soldier) = firebending_board(0, P1);
    runner
        .declare_attackers(&[(soldier, AttackTarget::Player(P0))])
        .expect("opponent attacker declaration succeeds");
    assert!(
        !matches!(runner.state().waiting_for, WaitingFor::OrderTriggers { .. }),
        "Firebender Ascension must not trigger for an opponent's creature"
    );
    runner.advance_until_stack_empty();

    assert_eq!(quest_count(runner.state(), firebender), 0);
    assert_eq!(red_count(runner.state(), P1.0 as usize), 1);
}

#[test]
fn attack_triggers_are_placed_in_apnap_order() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let firebender = scenario
        .add_enchantment_from_oracle(P0, "Firebender Ascension", FIREBENDER_ORACLE)
        .id();
    let opponent_trigger = scenario
        .add_enchantment_from_oracle(P1, "Opponent Attack Trigger", OPPONENT_ATTACK_ORACLE)
        .id();
    let soldier = scenario
        .add_creature(P0, "Firebending Soldier", 2, 2)
        .from_oracle_text_with_keywords(&["firebending"], "Firebending 1")
        .id();
    let mut runner = scenario.build();
    runner.advance_to_combat();
    runner
        .declare_attackers(&[(soldier, AttackTarget::Player(P1))])
        .expect("attacker declaration succeeds");

    let WaitingFor::OrderTriggers { player, triggers } = runner.state().waiting_for.clone() else {
        panic!("active player's simultaneous triggers must be ordered");
    };
    assert_eq!(player, P0);
    let firebending_index = triggers
        .iter()
        .position(|trigger| trigger.source_id == soldier)
        .expect("Firebending trigger is present");
    let firebender_index = triggers
        .iter()
        .position(|trigger| trigger.source_id == firebender)
        .expect("Firebender Ascension trigger is present");
    runner
        .act(GameAction::OrderTriggers {
            order: vec![firebending_index, firebender_index],
        })
        .expect("active player's trigger order succeeds");

    assert_eq!(
        runner.state().stack.last().map(|entry| entry.source_id),
        Some(opponent_trigger),
        "nonactive player's trigger is placed above active player's triggers"
    );
}
