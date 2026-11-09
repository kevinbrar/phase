use std::sync::Arc;

use engine::game::deck_loading::DeckEntry;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::{GameAction, OutsideGameSelection};
use engine::types::card::CardFace;
use engine::types::game_state::{PlayerDeckPool, WaitingFor};
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::zones::Zone;

const ORACLE: &str = "You may reveal exactly two cards you own with different names from outside the game. An opponent chooses one of them. You put that card into your hand.";

fn sideboard_entry(name: &str, count: u32) -> DeckEntry {
    DeckEntry {
        card: CardFace {
            name: name.to_string(),
            ..Default::default()
        },
        count,
    }
}

fn setup(sideboard: Vec<DeckEntry>) -> (GameRunner, engine::types::identifiers::ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let spell = scenario
        .add_spell_to_hand_from_oracle(P0, "Extrapolate the Impossible", false, ORACLE)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let mut runner = scenario.build();
    runner.state_mut().deck_pools = vec![PlayerDeckPool {
        player: P0,
        current_sideboard: Arc::new(sideboard),
        ..Default::default()
    }];
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };
    (runner, spell)
}

fn resolve_extrapolate(runner: &mut GameRunner, spell: engine::types::identifiers::ObjectId) {
    runner.cast(spell).commit().stop_at_optional().resolve();
    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::OptionalEffectChoice { .. }
        ),
        "the spell's You may must be offered before revealing"
    );
    runner
        .act(GameAction::DecideOptionalEffect { accept: true })
        .expect("accept the optional reveal");
    runner.advance_until_stack_empty();
}

#[test]
fn fewer_than_two_different_names_cannot_reveal_a_partial_pair() {
    let (mut runner, spell) = setup(vec![sideboard_entry("Lightning Bolt", 2)]);
    resolve_extrapolate(&mut runner, spell);

    assert!(
        !matches!(
            runner.state().waiting_for,
            WaitingFor::OutsideGameChoice { .. }
        ),
        "one eligible name cannot satisfy the exactly-two, different-names requirement"
    );
    assert!(runner.state().players[0].hand.is_empty());
    assert!(runner.state().outside_game_cards_brought_in.is_empty());
    assert_eq!(runner.state().deck_pools[0].current_sideboard[0].count, 2);
}

#[test]
fn caster_reveals_two_distinct_names_then_opponent_puts_only_one_into_hand() {
    let (mut runner, spell) = setup(vec![
        sideboard_entry("Lightning Bolt", 2),
        sideboard_entry("Shock", 1),
        sideboard_entry("Opt", 1),
    ]);
    resolve_extrapolate(&mut runner, spell);

    let (choices, count, player) = match &runner.state().waiting_for {
        WaitingFor::OutsideGameChoice {
            choices,
            count,
            player,
            ..
        } => (choices, count, player),
        other => panic!("expected the caster's two-card outside-game selection, got {other:?}"),
    };
    assert_eq!(*player, P0);
    assert_eq!(*count, 2);
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Lightning Bolt", "Shock", "Opt"]
    );

    assert!(
        runner
            .act(GameAction::ChooseOutsideGameCards {
                selections: vec![OutsideGameSelection::Sideboard { sideboard_index: 0 }],
            })
            .is_err(),
        "the caster must select exactly two cards"
    );
    assert!(
        runner
            .act(GameAction::ChooseOutsideGameCards {
                selections: vec![
                    OutsideGameSelection::Sideboard { sideboard_index: 0 },
                    OutsideGameSelection::Sideboard { sideboard_index: 0 },
                ],
            })
            .is_err(),
        "a duplicated name cannot satisfy the different-names requirement"
    );

    runner
        .act(GameAction::ChooseOutsideGameCards {
            selections: vec![
                OutsideGameSelection::Sideboard { sideboard_index: 0 },
                OutsideGameSelection::Sideboard { sideboard_index: 1 },
            ],
        })
        .expect("reveal two differently named cards");

    match &runner.state().waiting_for {
        WaitingFor::OutsideGameChoice {
            player,
            choices,
            count,
            ..
        } => {
            assert_eq!(*player, P1, "an opponent chooses from the revealed pair");
            assert_eq!(*count, 1);
            assert_eq!(
                choices
                    .iter()
                    .map(|choice| choice.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Lightning Bolt", "Shock"]
            );
        }
        other => panic!("expected the opponent's choice, got {other:?}"),
    }

    runner
        .act(GameAction::ChooseOutsideGameCards {
            selections: vec![OutsideGameSelection::Sideboard { sideboard_index: 1 }],
        })
        .expect("opponent chooses Shock");

    assert_eq!(
        runner.state().players[0]
            .hand
            .iter()
            .filter_map(|id| runner
                .state()
                .objects
                .get(id)
                .map(|object| object.name.as_str()))
            .collect::<Vec<_>>(),
        vec!["Shock"]
    );
    assert_eq!(runner.state().outside_game_cards_brought_in.len(), 1);
    assert_eq!(
        runner.state().outside_game_cards_brought_in[0].sideboard_index,
        1
    );
    assert_eq!(runner.state().deck_pools[0].current_sideboard[0].count, 2);
    assert_eq!(runner.state().deck_pools[0].current_sideboard[1].count, 1);
    assert_eq!(runner.state().deck_pools[0].current_sideboard[2].count, 1);
    assert_eq!(runner.state().objects[&spell].zone, Zone::Graveyard);
}
