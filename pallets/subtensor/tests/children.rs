#![allow(clippy::indexing_slicing)]
use crate::mock::*;
use frame_support::{assert_err, assert_noop, assert_ok};
mod mock;
use pallet_subtensor::{utils::TransactionType, *};
use sp_core::U256;

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_success --exact --nocapture
#[test]
fn test_do_set_child_singular_success() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child)]
        ));

        // Verify child assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion, child)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_network_does_not_exist --exact --nocapture
#[test]
fn test_do_set_child_singular_network_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 999; // Non-existent network
        let proportion: u64 = 1000;

        // Attempt to set child
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child)]
            ),
            Error::<Test>::SubNetworkDoesNotExist
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_invalid_child --exact --nocapture
#[test]
fn test_do_set_child_singular_invalid_child() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Attempt to set child as the same hotkey
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![
                    (proportion, hotkey) // Invalid child
                ]
            ),
            Error::<Test>::InvalidChild
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_non_associated_coldkey --exact --nocapture
#[test]
fn test_do_set_child_singular_non_associated_coldkey() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey with a different coldkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, U256::from(999), 0);

        // Attempt to set child
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child)]
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_root_network --exact --nocapture
#[test]
fn test_do_set_child_singular_root_network() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = SubtensorModule::get_root_netuid(); // Root network
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);

        // Attempt to set child
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child)]
            ),
            Error::<Test>::RegistrationNotPermittedOnRootSubnet
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_old_children_cleanup --exact --nocapture
#[test]
fn test_do_set_child_singular_old_children_cleanup() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let old_child = U256::from(3);
        let new_child = U256::from(4);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set old child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, old_child)]
        ));

        // Set new child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, new_child)]
        ));

        // Verify old child is removed
        let old_child_parents = SubtensorModule::get_parents(&old_child, netuid);
        assert!(old_child_parents.is_empty());

        // Verify new child assignment
        let new_child_parents = SubtensorModule::get_parents(&new_child, netuid);
        assert_eq!(new_child_parents, vec![(proportion, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_old_children_cleanup --exact --nocapture
#[test]
fn test_do_set_child_singular_new_children_assignment() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child)]
        ));

        // Verify child assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion, child)]);

        // Verify parent assignment
        let parents = SubtensorModule::get_parents(&child, netuid);
        assert_eq!(parents, vec![(proportion, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_proportion_edge_cases --exact --nocapture
#[test]
fn test_do_set_child_singular_proportion_edge_cases() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set child with minimum proportion
        let min_proportion: u64 = 0;
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(min_proportion, child)]
        ));

        // Verify child assignment with minimum proportion
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(min_proportion, child)]);

        // Set child with maximum proportion
        let max_proportion: u64 = u64::MAX;
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(max_proportion, child)]
        ));

        // Verify child assignment with maximum proportion
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(max_proportion, child)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_child_singular_multiple_children --exact --nocapture
#[test]
fn test_do_set_child_singular_multiple_children() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 1;
        let proportion1: u64 = 500;
        let proportion2: u64 = 500;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set first child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion1, child1)]
        ));

        // Set second child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion2, child2)]
        ));

        // Verify children assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion2, child2)]);

        // Verify parent assignment for both children
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert!(parents1.is_empty()); // Old child should be removed

        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert_eq!(parents2, vec![(proportion2, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_add_singular_child --exact --nocapture
#[test]
fn test_add_singular_child() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;
        let child = U256::from(1);
        let hotkey = U256::from(1);
        let coldkey = U256::from(2);
        assert_eq!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX, child)]
            ),
            Err(Error::<Test>::SubNetworkDoesNotExist.into())
        );
        add_network(netuid, 0, 0);
        assert_eq!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX, child)]
            ),
            Err(Error::<Test>::NonAssociatedColdKey.into())
        );
        SubtensorModule::create_account_if_non_existent(&coldkey, &hotkey);
        assert_eq!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX, child)]
            ),
            Err(Error::<Test>::InvalidChild.into())
        );
        let child = U256::from(3);
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(u64::MAX, child)]
        ));
    })
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_get_stake_for_hotkey_on_subnet --exact --nocapture
#[test]
fn test_get_stake_for_hotkey_on_subnet() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;
        let hotkey0 = U256::from(1);
        let hotkey1 = U256::from(2);
        let coldkey0 = U256::from(3);
        let coldkey1 = U256::from(4);

        add_network(netuid, 0, 0);

        let max_stake: u64 = 3000;
        SubtensorModule::set_network_max_stake(netuid, max_stake);

        SubtensorModule::create_account_if_non_existent(&coldkey0, &hotkey0);
        SubtensorModule::create_account_if_non_existent(&coldkey1, &hotkey1);

        SubtensorModule::increase_stake_on_coldkey_hotkey_account(&coldkey0, &hotkey0, 1000);
        SubtensorModule::increase_stake_on_coldkey_hotkey_account(&coldkey0, &hotkey1, 1000);
        SubtensorModule::increase_stake_on_coldkey_hotkey_account(&coldkey1, &hotkey0, 1000);
        SubtensorModule::increase_stake_on_coldkey_hotkey_account(&coldkey1, &hotkey1, 1000);

        assert_eq!(SubtensorModule::get_total_stake_for_hotkey(&hotkey0), 2000);
        assert_eq!(SubtensorModule::get_total_stake_for_hotkey(&hotkey1), 2000);

        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey0, netuid),
            2000
        );
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey1, netuid),
            2000
        );

        // Set child relationship
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey0),
            hotkey0,
            netuid,
            vec![(u64::MAX, hotkey1)]
        ));

        // Check stakes after setting child
        let stake0 = SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey0, netuid);
        let stake1 = SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey1, netuid);

        assert_eq!(stake0, 0);
        assert_eq!(stake1, max_stake);

        // Change child relationship to 50%
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey0),
            hotkey0,
            netuid,
            vec![(u64::MAX / 2, hotkey1)]
        ));

        // Check stakes after changing child relationship
        let stake0 = SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey0, netuid);
        let stake1 = SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey1, netuid);

        assert_eq!(stake0, 1001);
        assert!(stake1 >= max_stake - 1 && stake1 <= max_stake);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_child_singular_success --exact --nocapture
#[test]
fn test_do_revoke_child_singular_success() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child)]
        ));

        // Verify child assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion, child)]);

        // Revoke child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify child removal
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());

        // Verify parent removal
        let parents = SubtensorModule::get_parents(&child, netuid);
        assert!(parents.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_child_singular_network_does_not_exist --exact --nocapture
#[test]
fn test_do_revoke_child_singular_network_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 999; // Non-existent network

        // Attempt to revoke child
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![]
            ),
            Error::<Test>::SubNetworkDoesNotExist
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_child_singular_non_associated_coldkey --exact --nocapture
#[test]
fn test_do_revoke_child_singular_non_associated_coldkey() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;

        // Add network and register hotkey with a different coldkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, U256::from(999), 0);

        // Attempt to revoke child
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![]
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_child_singular_child_not_associated --exact --nocapture
#[test]
fn test_do_revoke_child_singular_child_not_associated() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        // Attempt to revoke child that is not associated
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX, child)]
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_success --exact --nocapture
#[test]
fn test_do_set_children_multiple_success() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 1;
        let proportion1: u64 = 1000;
        let proportion2: u64 = 2000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set multiple children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion1, child1), (proportion2, child2)]
        ));

        // Verify children assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion1, child1), (proportion2, child2)]);

        // Verify parent assignment for both children
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert_eq!(parents1, vec![(proportion1, hotkey)]);

        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert_eq!(parents2, vec![(proportion2, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_network_does_not_exist --exact --nocapture
#[test]
fn test_do_set_children_multiple_network_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let netuid: u16 = 999; // Non-existent network
        let proportion: u64 = 1000;

        // Attempt to set children
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child1)]
            ),
            Error::<Test>::SubNetworkDoesNotExist
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_invalid_child --exact --nocapture
#[test]
fn test_do_set_children_multiple_invalid_child() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Attempt to set child as the same hotkey
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, hotkey)]
            ),
            Error::<Test>::InvalidChild
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_non_associated_coldkey --exact --nocapture
#[test]
fn test_do_set_children_multiple_non_associated_coldkey() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey with a different coldkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, U256::from(999), 0);

        // Attempt to set children
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child)]
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_root_network --exact --nocapture
#[test]
fn test_do_set_children_multiple_root_network() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = SubtensorModule::get_root_netuid(); // Root network
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);

        // Attempt to set children
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(proportion, child)]
            ),
            Error::<Test>::RegistrationNotPermittedOnRootSubnet
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_old_children_cleanup --exact --nocapture
#[test]
fn test_do_set_children_multiple_old_children_cleanup() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let old_child = U256::from(3);
        let new_child1 = U256::from(4);
        let new_child2 = U256::from(5);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set old child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, old_child)]
        ));

        // Set new children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, new_child1), (proportion, new_child2)]
        ));

        // Verify old child is removed
        let old_child_parents = SubtensorModule::get_parents(&old_child, netuid);
        assert!(old_child_parents.is_empty());

        // Verify new children assignment
        let new_child1_parents = SubtensorModule::get_parents(&new_child1, netuid);
        assert_eq!(new_child1_parents, vec![(proportion, hotkey)]);

        let new_child2_parents = SubtensorModule::get_parents(&new_child2, netuid);
        assert_eq!(new_child2_parents, vec![(proportion, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_proportion_edge_cases --exact --nocapture
#[test]
fn test_do_set_children_multiple_proportion_edge_cases() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set children with minimum and maximum proportions
        let min_proportion: u64 = 0;
        let max_proportion: u64 = u64::MAX;
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(min_proportion, child1), (max_proportion, child2)]
        ));

        // Verify children assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(
            children,
            vec![(min_proportion, child1), (max_proportion, child2)]
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_set_children_multiple_overwrite_existing --exact --nocapture
#[test]
fn test_do_set_children_multiple_overwrite_existing() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let child3 = U256::from(5);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set initial children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child1), (proportion, child2)]
        ));

        // Overwrite with new children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion * 2, child2), (proportion * 3, child3)]
        ));

        // Verify final children assignment
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(
            children,
            vec![(proportion * 2, child2), (proportion * 3, child3)]
        );

        // Verify parent assignment for all children
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert!(parents1.is_empty());

        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert_eq!(parents2, vec![(proportion * 2, hotkey)]);

        let parents3 = SubtensorModule::get_parents(&child3, netuid);
        assert_eq!(parents3, vec![(proportion * 3, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_childkey_take_functionality --exact --nocapture
#[test]
fn test_childkey_take_functionality() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Test default and max childkey take
        let default_take = SubtensorModule::get_default_childkey_take();
        let max_take = SubtensorModule::get_max_childkey_take();
        log::info!("Default take: {}, Max take: {}", default_take, max_take);

        // Check if default take and max take are the same
        assert_eq!(
            default_take, max_take,
            "Default take should be equal to max take"
        );

        // Log the actual value of MaxChildkeyTake
        log::info!(
            "MaxChildkeyTake value: {:?}",
            MaxChildkeyTake::<Test>::get()
        );

        // Test setting childkey take
        let new_take: u16 = max_take / 2; // 50% of max_take
        assert_ok!(SubtensorModule::set_childkey_take(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            new_take
        ));

        // Verify childkey take was set correctly
        let stored_take = SubtensorModule::get_childkey_take(&hotkey, netuid);
        log::info!("Stored take: {}", stored_take);
        assert_eq!(stored_take, new_take);

        // Test setting childkey take outside of allowed range
        let invalid_take: u16 = max_take + 1;
        assert_noop!(
            SubtensorModule::set_childkey_take(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                invalid_take
            ),
            Error::<Test>::InvalidChildkeyTake
        );

        // Test setting childkey take with non-associated coldkey
        let non_associated_coldkey = U256::from(999);
        assert_noop!(
            SubtensorModule::set_childkey_take(
                RuntimeOrigin::signed(non_associated_coldkey),
                hotkey,
                netuid,
                new_take
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_childkey_take_rate_limiting --exact --nocapture
#[test]
fn test_childkey_take_rate_limiting() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set a rate limit for childkey take changes
        let rate_limit: u64 = 100;
        SubtensorModule::set_tx_childkey_take_rate_limit(rate_limit);

        log::info!(
            "TxChildkeyTakeRateLimit: {:?}",
            TxChildkeyTakeRateLimit::<Test>::get()
        );

        // First transaction (should succeed)
        assert_ok!(SubtensorModule::set_childkey_take(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            500
        ));

        let current_block = SubtensorModule::get_current_block_as_u64();
        let last_block = SubtensorModule::get_last_transaction_block(
            &hotkey,
            netuid,
            &TransactionType::SetChildkeyTake,
        );
        log::info!(
            "After first transaction: current_block: {}, last_block: {}",
            current_block,
            last_block
        );

        // Second transaction (should fail due to rate limit)
        let result =
            SubtensorModule::set_childkey_take(RuntimeOrigin::signed(coldkey), hotkey, netuid, 600);
        log::info!("Second transaction result: {:?}", result);
        let current_block = SubtensorModule::get_current_block_as_u64();
        let last_block = SubtensorModule::get_last_transaction_block(
            &hotkey,
            netuid,
            &TransactionType::SetChildkeyTake,
        );
        log::info!(
            "After second transaction attempt: current_block: {}, last_block: {}",
            current_block,
            last_block
        );

        assert_noop!(result, Error::<Test>::TxChildkeyTakeRateLimitExceeded);

        // Advance the block number to just before the rate limit
        run_to_block(rate_limit);

        // Third transaction (should still fail)
        let result =
            SubtensorModule::set_childkey_take(RuntimeOrigin::signed(coldkey), hotkey, netuid, 650);
        log::info!("Third transaction result: {:?}", result);
        let current_block = SubtensorModule::get_current_block_as_u64();
        let last_block = SubtensorModule::get_last_transaction_block(
            &hotkey,
            netuid,
            &TransactionType::SetChildkeyTake,
        );
        log::info!(
            "After third transaction attempt: current_block: {}, last_block: {}",
            current_block,
            last_block
        );

        assert_noop!(result, Error::<Test>::TxChildkeyTakeRateLimitExceeded);

        // Advance the block number to just after the rate limit
        run_to_block(rate_limit * 2);

        // Fourth transaction (should succeed)
        assert_ok!(SubtensorModule::set_childkey_take(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            700
        ));

        let current_block = SubtensorModule::get_current_block_as_u64();
        let last_block = SubtensorModule::get_last_transaction_block(
            &hotkey,
            netuid,
            &TransactionType::SetChildkeyTake,
        );
        log::info!(
            "After fourth transaction: current_block: {}, last_block: {}",
            current_block,
            last_block
        );

        // Verify the final take was set
        let stored_take = SubtensorModule::get_childkey_take(&hotkey, netuid);
        assert_eq!(stored_take, 700);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_multiple_networks_childkey_take --exact --nocapture
#[test]
fn test_multiple_networks_childkey_take() {
    new_test_ext(1).execute_with(|| {
        const NUM_NETWORKS: u16 = 10;
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);

        // Create 10 networks and set up neurons (skip network 0)
        for netuid in 1..NUM_NETWORKS {
            // Add network
            add_network(netuid, 13, 0);

            // Register neuron
            register_ok_neuron(netuid, hotkey, coldkey, 0);

            // Set a unique childkey take value for each network
            let take_value = (netuid + 1) * 1000; // Values will be 1000, 2000, ..., 10000
            assert_ok!(SubtensorModule::set_childkey_take(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                take_value
            ));

            // Verify the childkey take was set correctly
            let stored_take = SubtensorModule::get_childkey_take(&hotkey, netuid);
            assert_eq!(
                stored_take, take_value,
                "Childkey take not set correctly for network {}",
                netuid
            );

            // Log the set value
            log::info!("Network {}: Childkey take set to {}", netuid, take_value);
        }

        // Verify all networks have different childkey take values
        for i in 1..NUM_NETWORKS {
            for j in (i + 1)..NUM_NETWORKS {
                let take_i = SubtensorModule::get_childkey_take(&hotkey, i);
                let take_j = SubtensorModule::get_childkey_take(&hotkey, j);
                assert_ne!(
                    take_i, take_j,
                    "Childkey take values should be different for networks {} and {}",
                    i, j
                );
            }
        }
    });
}

#[test]
fn test_do_set_children_multiple_empty_list() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set empty children list
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify children assignment is empty
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_success --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_success() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 1;
        let proportion1: u64 = 1000;
        let proportion2: u64 = 2000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set multiple children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion1, child1), (proportion2, child2)]
        ));

        // Revoke multiple children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify children removal
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());

        // Verify parent removal for both children
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert!(parents1.is_empty());

        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert!(parents2.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_network_does_not_exist --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_network_does_not_exist() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 999; // Non-existent network
                               // Attempt to revoke children
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX / 2, child1), (u64::MAX / 2, child2)]
            ),
            Error::<Test>::SubNetworkDoesNotExist
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_non_associated_coldkey --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_non_associated_coldkey() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let netuid: u16 = 1;

        // Add network and register hotkey with a different coldkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, U256::from(999), 0);

        // Attempt to revoke children
        assert_err!(
            SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkey,
                netuid,
                vec![(u64::MAX / 2, child1), (u64::MAX / 2, child2)]
            ),
            Error::<Test>::NonAssociatedColdKey
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_partial_revocation --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_partial_revocation() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let child3 = U256::from(5);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set multiple children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![
                (proportion, child1),
                (proportion, child2),
                (proportion, child3)
            ]
        ));

        // Revoke only child3
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child1), (proportion, child2)]
        ));

        // Verify children removal
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion, child1), (proportion, child2)]);

        // Verify parents.
        let parents1 = SubtensorModule::get_parents(&child3, netuid);
        assert!(parents1.is_empty());
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert_eq!(parents1, vec![(proportion, hotkey)]);
        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert_eq!(parents2, vec![(proportion, hotkey)]);
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_non_existent_children --exact --nocapture

#[test]
fn test_do_revoke_children_multiple_non_existent_children() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set one child
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child1)]
        ));

        // Attempt to revoke existing and non-existent children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify all children are removed
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());

        // Verify parent removal for the existing child
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert!(parents1.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_empty_list --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_empty_list() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let netuid: u16 = 1;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Attempt to revoke with an empty list
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify no changes in children
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_do_revoke_children_multiple_complex_scenario --exact --nocapture
#[test]
fn test_do_revoke_children_multiple_complex_scenario() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let child3 = U256::from(5);
        let netuid: u16 = 1;
        let proportion1: u64 = 1000;
        let proportion2: u64 = 2000;
        let proportion3: u64 = 3000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set multiple children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![
                (proportion1, child1),
                (proportion2, child2),
                (proportion3, child3)
            ]
        ));

        // Revoke child2
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion1, child1), (proportion3, child3)]
        ));

        // Verify remaining children
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert_eq!(children, vec![(proportion1, child1), (proportion3, child3)]);

        // Verify parent removal for child2
        let parents2 = SubtensorModule::get_parents(&child2, netuid);
        assert!(parents2.is_empty());

        // Revoke remaining children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify all children are removed
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());

        // Verify parent removal for all children
        let parents1 = SubtensorModule::get_parents(&child1, netuid);
        assert!(parents1.is_empty());
        let parents3 = SubtensorModule::get_parents(&child3, netuid);
        assert!(parents3.is_empty());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_get_network_max_stake --exact --nocapture
#[test]
fn test_get_network_max_stake() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;
        let default_max_stake = SubtensorModule::get_network_max_stake(netuid);

        // Check that the default value is set correctly
        assert_eq!(default_max_stake, 500_000_000_000_000);

        // Set a new max stake value
        let new_max_stake: u64 = 1_000_000;
        SubtensorModule::set_network_max_stake(netuid, new_max_stake);

        // Check that the new value is retrieved correctly
        assert_eq!(
            SubtensorModule::get_network_max_stake(netuid),
            new_max_stake
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_set_network_max_stake --exact --nocapture
#[test]
fn test_set_network_max_stake() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;
        let initial_max_stake = SubtensorModule::get_network_max_stake(netuid);

        // Set a new max stake value
        let new_max_stake: u64 = 500_000;
        SubtensorModule::set_network_max_stake(netuid, new_max_stake);

        // Check that the new value is set correctly
        assert_eq!(
            SubtensorModule::get_network_max_stake(netuid),
            new_max_stake
        );
        assert_ne!(
            SubtensorModule::get_network_max_stake(netuid),
            initial_max_stake
        );

        // Check that the event is emitted
        System::assert_last_event(Event::NetworkMaxStakeSet(netuid, new_max_stake).into());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_set_network_max_stake_multiple_networks --exact --nocapture
#[test]
fn test_set_network_max_stake_multiple_networks() {
    new_test_ext(1).execute_with(|| {
        let netuid1: u16 = 1;
        let netuid2: u16 = 2;

        // Set different max stake values for two networks
        let max_stake1: u64 = 1_000_000;
        let max_stake2: u64 = 2_000_000;
        SubtensorModule::set_network_max_stake(netuid1, max_stake1);
        SubtensorModule::set_network_max_stake(netuid2, max_stake2);

        // Check that the values are set correctly for each network
        assert_eq!(SubtensorModule::get_network_max_stake(netuid1), max_stake1);
        assert_eq!(SubtensorModule::get_network_max_stake(netuid2), max_stake2);
        assert_ne!(
            SubtensorModule::get_network_max_stake(netuid1),
            SubtensorModule::get_network_max_stake(netuid2)
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_set_network_max_stake_update --exact --nocapture
#[test]
fn test_set_network_max_stake_update() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;

        // Set an initial max stake value
        let initial_max_stake: u64 = 1_000_000;
        SubtensorModule::set_network_max_stake(netuid, initial_max_stake);

        // Update the max stake value
        let updated_max_stake: u64 = 1_500_000;
        SubtensorModule::set_network_max_stake(netuid, updated_max_stake);

        // Check that the value is updated correctly
        assert_eq!(
            SubtensorModule::get_network_max_stake(netuid),
            updated_max_stake
        );
        assert_ne!(
            SubtensorModule::get_network_max_stake(netuid),
            initial_max_stake
        );

        // Check that the event is emitted for the update
        System::assert_last_event(Event::NetworkMaxStakeSet(netuid, updated_max_stake).into());
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_children_stake_values --exact --nocapture
#[test]
fn test_children_stake_values() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child1 = U256::from(3);
        let child2 = U256::from(4);
        let child3 = U256::from(5);
        let netuid: u16 = 1;
        let proportion1: u64 = u64::MAX / 4;
        let proportion2: u64 = u64::MAX / 4;
        let proportion3: u64 = u64::MAX / 4;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        SubtensorModule::set_max_registrations_per_block(netuid, 4);
        SubtensorModule::set_target_registrations_per_interval(netuid, 4);
        register_ok_neuron(netuid, hotkey, coldkey, 0);
        register_ok_neuron(netuid, child1, coldkey, 0);
        register_ok_neuron(netuid, child2, coldkey, 0);
        register_ok_neuron(netuid, child3, coldkey, 0);
        SubtensorModule::increase_stake_on_coldkey_hotkey_account(
            &coldkey,
            &hotkey,
            100_000_000_000_000,
        );

        // Set multiple children with proportions.
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![
                (proportion1, child1),
                (proportion2, child2),
                (proportion3, child3)
            ]
        ));
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey, netuid),
            25_000_000_069_852
        );
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&child1, netuid),
            24_999_999_976_716
        );
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&child2, netuid),
            24_999_999_976_716
        );
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&child3, netuid),
            24_999_999_976_716
        );
        assert_eq!(
            SubtensorModule::get_stake_for_hotkey_on_subnet(&child3, netuid)
                + SubtensorModule::get_stake_for_hotkey_on_subnet(&child2, netuid)
                + SubtensorModule::get_stake_for_hotkey_on_subnet(&child1, netuid)
                + SubtensorModule::get_stake_for_hotkey_on_subnet(&hotkey, netuid),
            100_000_000_000_000
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_get_parents_chain --exact --nocapture
#[test]
fn test_get_parents_chain() {
    new_test_ext(1).execute_with(|| {
        let netuid: u16 = 1;
        let coldkey = U256::from(1);
        let num_keys: usize = 5;
        let proportion = u64::MAX / 2; // 50% stake allocation

        log::info!(
            "Test setup: netuid={}, coldkey={}, num_keys={}, proportion={}",
            netuid,
            coldkey,
            num_keys,
            proportion
        );

        // Create a vector of hotkeys
        let hotkeys: Vec<U256> = (0..num_keys).map(|i| U256::from(i as u64 + 2)).collect();
        log::info!("Created hotkeys: {:?}", hotkeys);

        // Add network
        add_network(netuid, 13, 0);
        SubtensorModule::set_max_registrations_per_block(netuid, 1000);
        SubtensorModule::set_target_registrations_per_interval(netuid, 1000);
        log::info!("Network added and parameters set: netuid={}", netuid);

        // Register all neurons
        for hotkey in &hotkeys {
            register_ok_neuron(netuid, *hotkey, coldkey, 0);
            log::info!(
                "Registered neuron: hotkey={}, coldkey={}, netuid={}",
                hotkey,
                coldkey,
                netuid
            );
        }

        // Set up parent-child relationships
        for i in 0..num_keys - 1 {
            assert_ok!(SubtensorModule::do_set_children(
                RuntimeOrigin::signed(coldkey),
                hotkeys[i],
                netuid,
                vec![(proportion, hotkeys[i + 1])]
            ));
            log::info!(
                "Set parent-child relationship: parent={}, child={}, proportion={}",
                hotkeys[i],
                hotkeys[i + 1],
                proportion
            );
        }

        // Test get_parents for each hotkey
        for i in 1..num_keys {
            let parents = SubtensorModule::get_parents(&hotkeys[i], netuid);
            log::info!(
                "Testing get_parents for hotkey {}: {:?}",
                hotkeys[i],
                parents
            );
            assert_eq!(
                parents.len(),
                1,
                "Hotkey {} should have exactly one parent",
                i
            );
            assert_eq!(
                parents[0],
                (proportion, hotkeys[i - 1]),
                "Incorrect parent for hotkey {}",
                i
            );
        }

        // Test get_parents for the root (should be empty)
        let root_parents = SubtensorModule::get_parents(&hotkeys[0], netuid);
        log::info!(
            "Testing get_parents for root hotkey {}: {:?}",
            hotkeys[0],
            root_parents
        );
        assert!(
            root_parents.is_empty(),
            "Root hotkey should have no parents"
        );

        // Test multiple parents
        let last_hotkey = hotkeys[num_keys - 1];
        let new_parent = U256::from(num_keys as u64 + 2);
        register_ok_neuron(netuid, new_parent, coldkey, 0);
        log::info!(
            "Registered new parent neuron: new_parent={}, coldkey={}, netuid={}",
            new_parent,
            coldkey,
            netuid
        );

        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            new_parent,
            netuid,
            vec![(proportion / 2, last_hotkey)]
        ));
        log::info!(
            "Set additional parent-child relationship: parent={}, child={}, proportion={}",
            new_parent,
            last_hotkey,
            proportion / 2
        );

        let last_hotkey_parents = SubtensorModule::get_parents(&last_hotkey, netuid);
        log::info!(
            "Testing get_parents for last hotkey {} with multiple parents: {:?}",
            last_hotkey,
            last_hotkey_parents
        );
        assert_eq!(
            last_hotkey_parents.len(),
            2,
            "Last hotkey should have two parents"
        );
        assert!(
            last_hotkey_parents.contains(&(proportion, hotkeys[num_keys - 2])),
            "Last hotkey should still have its original parent"
        );
        assert!(
            last_hotkey_parents.contains(&(proportion / 2, new_parent)),
            "Last hotkey should have the new parent"
        );
    });
}

// SKIP_WASM_BUILD=1 RUST_LOG=info cargo test --test children -- test_childkey_take_removal_on_empty_children --exact --nocapture
#[test]
fn test_childkey_take_removal_on_empty_children() {
    new_test_ext(1).execute_with(|| {
        let coldkey = U256::from(1);
        let hotkey = U256::from(2);
        let child = U256::from(3);
        let netuid: u16 = 1;
        let proportion: u64 = 1000;

        // Add network and register hotkey
        add_network(netuid, 13, 0);
        register_ok_neuron(netuid, hotkey, coldkey, 0);

        // Set a child and childkey take
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![(proportion, child)]
        ));

        let take: u16 = u16::MAX / 10; // 10% take
        assert_ok!(SubtensorModule::set_childkey_take(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            take
        ));

        // Verify childkey take is set
        assert_eq!(SubtensorModule::get_childkey_take(&hotkey, netuid), take);

        // Remove all children
        assert_ok!(SubtensorModule::do_set_children(
            RuntimeOrigin::signed(coldkey),
            hotkey,
            netuid,
            vec![]
        ));

        // Verify children are removed
        let children = SubtensorModule::get_children(&hotkey, netuid);
        assert!(children.is_empty());

        // Verify childkey take is removed
        assert_eq!(SubtensorModule::get_childkey_take(&hotkey, netuid), 0);
        // Verify childkey take storage is empty
        assert_eq!(ChildkeyTake::<Test>::get(hotkey, netuid), 0);
    });
}
