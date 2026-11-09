use engine::game::scenario::{GameRunner, GameScenario, P0};
use engine::types::ability::TargetRef;
use engine::types::actions::GameAction;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::mana::ManaCost;
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

const ORACLE: &str = "Flying\nWhen Uldaros Theorix enters, if you cast him, exile up to one target nonland card of each card type from your graveyard. Copy those cards. You may cast any number of spells with total mana value 6 or less from among the copies without paying their mana costs. (Permanent spells cast this way become tokens.)";

struct Setup {
    runner: GameRunner,
    uldaros: ObjectId,
    instant: ObjectId,
    sorcery: ObjectId,
    other_sorcery: ObjectId,
    creature: ObjectId,
}

fn setup() -> Setup {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let uldaros = scenario
        .add_creature_to_hand_from_oracle(P0, "Uldaros Theorix", 6, 6, ORACLE)
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let instant = scenario
        .add_spell_to_graveyard(P0, "One-Mana Instant", true)
        .with_mana_cost(ManaCost::generic(1))
        .from_oracle_text("You gain 1 life.")
        .id();
    let sorcery = scenario
        .add_spell_to_graveyard(P0, "Two-Mana Sorcery", false)
        .with_mana_cost(ManaCost::generic(2))
        .from_oracle_text("You gain 2 life.")
        .id();
    let other_sorcery = scenario
        .add_spell_to_graveyard(P0, "Other Two-Mana Sorcery", false)
        .with_mana_cost(ManaCost::generic(2))
        .from_oracle_text("You gain 3 life.")
        .id();
    let creature = scenario
        .add_creature_to_graveyard(P0, "Four-Mana Creature", 3, 3)
        .with_mana_cost(ManaCost::generic(4))
        .id();
    let mut runner = scenario.build();
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };
    Setup {
        runner,
        uldaros,
        instant,
        sorcery,
        other_sorcery,
        creature,
    }
}

fn cast_uldaros_to_target_prompt(setup: &mut Setup) {
    let mut committed = setup.runner.cast(setup.uldaros).commit();
    for _ in 0..16 {
        match &committed.state().waiting_for {
            WaitingFor::TargetSelection { .. }
            | WaitingFor::TriggerTargetSelection { .. }
            | WaitingFor::MultiTargetSelection { .. } => return,
            WaitingFor::OrderTriggers { .. } => {
                engine::game::triggers::drain_order_triggers_with_identity(committed.state_mut());
            }
            WaitingFor::Priority { .. } if !committed.state().stack.is_empty() => {
                committed
                    .act(GameAction::PassPriority)
                    .expect("pass priority while resolving Uldaros");
            }
            waiting => panic!("expected Uldaros's ETB target prompt, got {waiting:?}"),
        }
    }
    panic!("Uldaros's ETB target prompt did not appear");
}

fn select_targets(setup: &mut Setup, targets: &[ObjectId]) {
    setup
        .runner
        .act(GameAction::SelectTargets {
            targets: targets.iter().copied().map(TargetRef::Object).collect(),
        })
        .expect("select legal, differently typed graveyard cards");
    setup.runner.advance_until_stack_empty();
}

fn p0_life(runner: &GameRunner) -> i32 {
    runner
        .state()
        .players
        .iter()
        .find(|player| player.id == PlayerId(0))
        .unwrap()
        .life
}

#[test]
fn casts_copies_for_free_and_enforces_the_total_mana_value_budget() {
    let mut setup = setup();
    let starting_life = p0_life(&setup.runner);
    cast_uldaros_to_target_prompt(&mut setup);

    assert!(
        matches!(
            setup.runner.state().waiting_for,
            WaitingFor::TargetSelection { .. } | WaitingFor::TriggerTargetSelection { .. }
        ),
        "casting Uldaros should put its cast-qualified ETB trigger through target selection, got {:?}",
        setup.runner.state().waiting_for
    );
    assert!(
        setup
            .runner
            .act(GameAction::SelectTargets {
                targets: vec![
                    TargetRef::Object(setup.sorcery),
                    TargetRef::Object(setup.other_sorcery),
                ],
            })
            .is_err(),
        "the target set cannot contain two cards of the same card type"
    );
    let targets = [setup.instant, setup.sorcery, setup.creature];
    select_targets(&mut setup, &targets);

    for original in [setup.instant, setup.sorcery, setup.creature] {
        assert_eq!(
            setup.runner.state().objects[&original].zone,
            Zone::Exile,
            "Uldaros exiles the selected originals before copying them"
        );
    }
    assert_eq!(
        setup.runner.state().objects[&setup.other_sorcery].zone,
        Zone::Graveyard,
        "an unselected card remains in the graveyard"
    );

    let offered = match &setup.runner.state().waiting_for {
        WaitingFor::ChooseFromZoneChoice { cards, .. } => cards,
        other => panic!("expected the copy-cast selection, got {other:?}"),
    };
    assert!(offered.contains(&setup.instant));
    assert!(offered.contains(&setup.sorcery));
    assert!(offered.contains(&setup.creature));
    assert!(
        setup
            .runner
            .act(GameAction::SelectCards {
                cards: vec![setup.instant, setup.sorcery, setup.creature],
            })
            .is_err(),
        "mana values 1 + 2 + 4 exceed Uldaros's total budget of 6"
    );

    setup
        .runner
        .act(GameAction::SelectCards {
            cards: vec![setup.sorcery, setup.creature],
        })
        .expect("select copies totaling exactly six mana value");
    setup.runner.advance_until_stack_empty();

    assert_eq!(
        p0_life(&setup.runner),
        starting_life + 2,
        "the sorcery copy resolves without paying its mana cost"
    );
    assert!(
        setup.runner.state().objects.values().any(|object| {
            object.name == "Four-Mana Creature"
                && object.is_token
                && object.zone == Zone::Battlefield
        }),
        "the permanent spell copy becomes a token"
    );
    assert_eq!(
        setup.runner.state().objects[&setup.instant].zone,
        Zone::Exile
    );
    assert_eq!(
        setup.runner.state().objects[&setup.sorcery].zone,
        Zone::Exile
    );
    assert_eq!(
        setup.runner.state().objects[&setup.creature].zone,
        Zone::Exile
    );
}

#[test]
fn choosing_no_copies_leaves_the_original_cards_exiled() {
    let mut setup = setup();
    let starting_life = p0_life(&setup.runner);
    cast_uldaros_to_target_prompt(&mut setup);
    let targets = [setup.instant, setup.creature];
    select_targets(&mut setup, &targets);

    setup
        .runner
        .act(GameAction::SelectCards { cards: vec![] })
        .expect("casting zero copies is allowed");
    setup.runner.advance_until_stack_empty();

    assert_eq!(p0_life(&setup.runner), starting_life);
    assert_eq!(
        setup.runner.state().objects[&setup.instant].zone,
        Zone::Exile
    );
    assert_eq!(
        setup.runner.state().objects[&setup.creature].zone,
        Zone::Exile
    );
    assert_eq!(
        setup.runner.state().objects[&setup.sorcery].zone,
        Zone::Graveyard
    );
}

#[test]
fn uldaros_put_onto_the_battlefield_without_being_cast_has_no_effect() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let uldaros = scenario
        .add_creature_to_graveyard(P0, "Uldaros Theorix", 6, 6)
        .from_oracle_text(ORACLE)
        .id();
    let reanimate = scenario
        .add_spell_to_hand_from_oracle(
            P0,
            "Reanimate Uldaros",
            true,
            "Return target creature card from your graveyard to the battlefield.",
        )
        .with_mana_cost(ManaCost::generic(0))
        .id();
    let instant = scenario
        .add_spell_to_graveyard(P0, "One-Mana Instant", true)
        .with_mana_cost(ManaCost::generic(1))
        .from_oracle_text("You gain 1 life.")
        .id();
    let mut runner = scenario.build();
    runner.state_mut().active_player = P0;
    runner.state_mut().priority_player = P0;
    runner.state_mut().waiting_for = WaitingFor::Priority { player: P0 };

    runner.cast(reanimate).target_object(uldaros).resolve();
    runner.advance_until_stack_empty();

    assert_eq!(runner.state().objects[&uldaros].zone, Zone::Battlefield);
    assert_eq!(runner.state().objects[&instant].zone, Zone::Graveyard);
    assert_eq!(p0_life(&runner), 20);
    assert!(
        !runner
            .state()
            .objects
            .values()
            .any(|object| object.name == "One-Mana Instant" && object.is_token),
        "Uldaros's copy ability must not resolve when it entered without being cast"
    );
    assert!(
        matches!(runner.state().waiting_for, WaitingFor::Priority { .. }),
        "the non-cast ETB must not leave a trigger-choice prompt: {:?}",
        runner.state().waiting_for
    );
}
