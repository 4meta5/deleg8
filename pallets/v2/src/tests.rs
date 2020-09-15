#![cfg(test)]

use super::*;
use frame_support::{
    assert_noop,
    assert_ok,
    impl_outer_event,
    impl_outer_origin,
    parameter_types,
    weights::Weight,
};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::IdentityLookup,
    Perbill,
};

pub type AccountId = u64;
pub type BlockNumber = u64;

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

mod delegate {
    pub use super::super::*;
}

impl_outer_event! {
    pub enum TestEvent for TestRuntime {
        frame_system<T>,
        pallet_balances<T>,
        delegate<T>,
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TestRuntime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for TestRuntime {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Call = ();
    type Hash = H256;
    type Hashing = ::sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = TestEvent;
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type AvailableBlockRatio = AvailableBlockRatio;
    type MaximumBlockLength = MaximumBlockLength;
    type Version = ();
    type ModuleToIndex = ();
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type BaseCallFilter = ();
    type SystemWeightInfo = ();
}
parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Trait for TestRuntime {
    type Balance = u64;
    type Event = TestEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}
parameter_types! {
    pub const Bond: u64 = 2;
    pub const MaxSize: u32 = 5;
    pub const MaxDepth: u32 = 3;
    pub const MaxKids: u32 = 3;
}
impl Trait for TestRuntime {
    type Event = TestEvent;
    type TreeId = u64;
    type Bond = Bond;
    type MaxSize = MaxSize;
    type MaxDepth = MaxDepth;
    type MaxKids = MaxKids;
    type Currency = Balances;
}
pub type System = frame_system::Module<TestRuntime>;
pub type Balances = pallet_balances::Module<TestRuntime>;
pub type Delegate = Module<TestRuntime>;

fn get_last_event() -> RawEvent<u64, u64, u64> {
    System::events()
        .into_iter()
        .map(|r| r.event)
        .filter_map(|e| {
            if let TestEvent::delegate(inner) = e {
                Some(inner)
            } else {
                None
            }
        })
        .last()
        .unwrap()
}

fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();
    pallet_balances::GenesisConfig::<TestRuntime> {
        balances: vec![
            (1, 1000),
            (2, 100),
            (3, 100),
            (4, 100),
            (5, 100),
            (6, 100),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
fn genesis_config_works() {
    new_test_ext().execute_with(|| {
        assert!(System::events().is_empty());
    });
}

#[test]
fn create_root_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Delegate::create_root(Origin::signed(21)),
            DispatchError::Module {
                index: 0,
                error: 3,
                message: Some("InsufficientBalance")
            }
        );
        assert_eq!(Balances::free_balance(&1), 1000);
        assert_ok!(Delegate::create_root(Origin::signed(1)));
        assert_eq!(RawEvent::RegisterIdRoot(0, 1, 2), get_last_event());
        assert_eq!(Balances::free_balance(&1), 998);
        for i in 2u64..7u64 {
            assert_eq!(Balances::free_balance(&i), 100);
            assert_ok!(Delegate::create_root(Origin::signed(i)));
            assert_eq!(
                RawEvent::RegisterIdRoot(i - 1u64, i, 2),
                get_last_event()
            );
            assert_eq!(Balances::free_balance(&i), 98);
        }
    });
}

#[test]
fn base_case_revoke_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Balances::free_balance(&1), 1000);
        assert_ok!(Delegate::create_root(Origin::signed(1)));
        assert_eq!(RawEvent::RegisterIdRoot(0, 1, 2), get_last_event());
        assert_eq!(Balances::free_balance(&1), 998);
        assert_ok!(Delegate::revoke(Origin::signed(1), 0, false));
        assert_eq!(RawEvent::RevokeDelegation(0), get_last_event());
        assert_eq!(Balances::free_balance(&1), 1000);
        for i in 2u64..7u64 {
            assert_eq!(Balances::free_balance(&i), 100);
            assert_ok!(Delegate::create_root(Origin::signed(i)));
            assert_eq!(
                RawEvent::RegisterIdRoot(i - 1u64, i, 2),
                get_last_event()
            );
            assert_eq!(Balances::free_balance(&i), 98);
            assert_ok!(Delegate::revoke(Origin::signed(i), i - 1u64, false));
            assert_eq!(RawEvent::RevokeDelegation(i - 1), get_last_event());
            assert_eq!(Balances::free_balance(&i), 100);
        }
    });
}

#[test]
fn add_remove_members_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Balances::free_balance(&1), 1000);
        assert_ok!(Delegate::create_root(Origin::signed(1)));
        assert_eq!(RawEvent::RegisterIdRoot(0, 1, 2), get_last_event());
        assert_eq!(Balances::free_balance(&1), 998);
        // this group would be above 5 (SIZE CONSTRAINT)
        assert_noop!(
            Delegate::add_members(Origin::signed(1), 0, vec![1, 2, 3, 4, 5, 6]),
            Error::<TestRuntime>::CannotAddGroupAboveMaxSize
        );
        // 1 + 5 = 5 <= 5 Module Group Size Limit
        assert_ok!(Delegate::add_members(
            Origin::signed(1),
            0,
            vec![2, 3, 4, 5]
        ));
        // Linear collateral requirement for adding members
        // 998 - 2 * (new_size) = 998 - 2 * 5 = 988
        assert_eq!(Balances::free_balance(&1), 988);
        assert_noop!(
            Delegate::remove_members(
                Origin::signed(2),
                0,
                vec![1, 3, 5],
                false,
            ),
            Error::<TestRuntime>::NotAuthorized
        );
        assert_ok!(Delegate::remove_members(
            Origin::signed(1),
            0,
            vec![2],
            false,
        ));
    });
}

#[test]
fn delegate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Delegate::create_root(Origin::signed(1)));
        assert_eq!(RawEvent::RegisterIdRoot(0, 1, 2), get_last_event());
        assert_eq!(Balances::free_balance(&1), 998);
        assert_ok!(Delegate::delegate(Origin::signed(1), 0, vec![2, 3, 4]));
        // 998 - bond ^ {height + kids} = 998 - 2 ^ {1 + 1}
        assert_eq!(Balances::free_balance(&1), 994);
        assert_eq!(RawEvent::DelegateBranch(0, 1, 1, 4), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(1), 0, vec![3, 4, 6]));
        // 994 - bond ^ {height + kids} = 994 - 2 ^ {1 + 2}
        assert_eq!(Balances::free_balance(&1), 986);
        assert_eq!(RawEvent::DelegateBranch(0, 2, 1, 8), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(2), 1, vec![3, 5]));
        // 100 - bond ^ {height + kids} = 100 - 2 ^ {2 + 1}
        assert_eq!(Balances::free_balance(&2), 92);
        assert_eq!(RawEvent::DelegateBranch(1, 3, 2, 8), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(3), 3, vec![1, 2]));
        // 100 - bond ^ {height + kids} = 100 - 2 ^ {3 + 1}
        assert_eq!(Balances::free_balance(&3), 84);
        assert_eq!(RawEvent::DelegateBranch(3, 4, 3, 16), get_last_event());
        // DEPTH CONSTRAINT
        assert_noop!(
            Delegate::delegate(Origin::signed(2), 4, vec![5, 6]),
            Error::<TestRuntime>::CannotDelegateBelowMaxDepth
        );
        assert_ok!(Delegate::delegate(Origin::signed(1), 0, vec![5, 6]));
        // 986 - bond ^ {height + kids} = 986 - 2 ^ {1 + 3}
        assert_eq!(Balances::free_balance(&1), 970);
        assert_eq!(RawEvent::DelegateBranch(0, 5, 1, 16), get_last_event());
        // SPAN CONSTRAINT
        assert_noop!(
            Delegate::delegate(Origin::signed(1), 0, vec![2, 8]),
            Error::<TestRuntime>::CannotDelegateAboveMaxKids
        );
    });
}

#[test]
fn recursive_revoke_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Delegate::create_root(Origin::signed(1)));
        assert_eq!(RawEvent::RegisterIdRoot(0, 1, 2), get_last_event());
        assert_eq!(Balances::free_balance(&1), 998);
        assert_ok!(Delegate::delegate(Origin::signed(1), 0, vec![2, 3, 4]));
        // 998 - bond ^ {height + kids} = 998 - 2 ^ {1 + 1}
        assert_eq!(Balances::free_balance(&1), 994);
        assert_eq!(RawEvent::DelegateBranch(0, 1, 1, 4), get_last_event());
        // 994 - bond ^ {height + kids} = 998 - 2 ^ {1 + 1}
        assert_eq!(Balances::free_balance(&1), 994);
        assert_eq!(RawEvent::DelegateBranch(0, 1, 1, 4), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(1), 0, vec![3, 4, 6]));
        // 994 - bond ^ {height + kids} = 994 - 2 ^ {1 + 2}
        assert_eq!(Balances::free_balance(&1), 986);
        assert_eq!(RawEvent::DelegateBranch(0, 2, 1, 8), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(2), 1, vec![3, 5]));
        // 100 - bond ^ {height + kids} = 100 - 2 ^ {2 + 1}
        assert_eq!(Balances::free_balance(&2), 92);
        assert_eq!(RawEvent::DelegateBranch(1, 3, 2, 8), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(3), 3, vec![1, 2]));
        // 100 - bond ^ {height + kids} = 100 - 2 ^ {3 + 1}
        assert_eq!(Balances::free_balance(&3), 84);
        assert_eq!(RawEvent::DelegateBranch(3, 4, 3, 16), get_last_event());
        assert_ok!(Delegate::delegate(Origin::signed(5), 3, vec![1, 2]));
        // 100 - bond ^ {height + kids} = 100 - 2 ^ {3 + 2}
        assert_eq!(Balances::free_balance(&5), 68);
        assert_eq!(RawEvent::DelegateBranch(3, 5, 5, 32), get_last_event());
        assert_ok!(Delegate::revoke(Origin::signed(1), 0, false));
        assert_eq!(Balances::free_balance(&5), 100);
        assert_eq!(Balances::free_balance(&3), 100);
        assert_eq!(Balances::free_balance(&1), 1000);
        assert_eq!(Balances::free_balance(&2), 100);
    });
}
