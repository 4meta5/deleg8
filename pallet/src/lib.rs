#![recursion_limit = "256"]
//! # Delegate Module
//! This module demonstrates an approach for bounding runtime
//! recursion. In particular, it places module-level constraints on
//! * the size of each group
//! * the number of subgroups
//! * the depth of delegation
//! These constraints allow us to use recursion in the module
//! with strict bounds on worst-case complexity.
//!
//! - [`delegate::Trait`](./trait.Trait.html)
//! - [`Call`](./enum.Call.html)
//!
//! ## Overview
//! This pallet allows delegation to a bounded depth. The incentives discourage
//! (1) adding members to large groups
//! (2) delegating to a new subgroup
//! The module enforces strict bounds on the size of a group, the number of
//! subgroups, and the depth of delegation to place explicit bounds on runtime
//! recursion. Each group registered on-chain has a `TreeId`. To get the state
//! of a group, we use the `Trees` map
//! ```rust, ignore
//! map TreeId => Option<TreeState<T::TreeId, T::AccountId>>;
//! ```
//! The `TreeState<_, _>` struct contains the data relevant to the bounds that
//! the module places on length, width and depth.
//! ```rust, ignore
//! pub struct TreeState<TreeId, AccountId> {
//!     pub id: TreeId,
//!     pub parent: Option<TreeId>,
//!     pub bonded: AccountId,
//!     pub height: u32,
//!     pub kids: u32,
//!     pub size: u32,
//! }
//! ```
//! The module's runtime configuration sets the maximum depth (`height`),
//! number of subgroups (`kids`), and number of members (`size`). Each
//! `TreeState<_, _>` is either a root or the child of a parent tree.
//! We define an algorithm for tree creation.
//! ```ignore
//! TreeCreation(parent: TreeState<_, _>)
//!     let kid = TreeState {
//!         parent: Some(parent.id)
//!         height: parent.height + 1u32,
//!         ..
//!     }
//!     parent.kids += 1u32;
//!     Storage::insert(kid);
//!     Storage::insert(parent)
//! ```
//! Before this algorithm is called, the runtime methods verify that
//! the kid's height does not exceed the `Trait::MaxDepth` and the parent's
//! kid count does not exceed `Trait::MaxKids`.
//!
//! Similarly, the runtime verifies that the new group count is below
//! the `Trait::MaxSize` before adding new members to the set of `AccountId`
//! associated on-chain with the group `TreeId`.
//!
//! ## Incentives
//! The bounds described above are not good enough. The variance of cost for
//! tree deletion is high because it is recursive and high variance poses a
//! problem. Although this recursion is bounded by the constraints, it can
//! still be very expensive if the overall Tree explores the limits of the
//! constraints and revokes the highest root. With this scenario in mind, the
//! incentives should discourage large groups and wide or deep delegation.
//!
//! ### Bonds for Adding Members Scales Linearly With Group Size
//!
//! ### Bonds for Delegating to Tree Scales Exponentially With Depth and Span
//!
//! [`Call`]: ./enum.Call.html
//! [`Trait`]: ./trait.Trait.html
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use frame_support::{
    decl_error,
    decl_event,
    decl_module,
    decl_storage,
    dispatch::DispatchError,
    ensure,
    storage::IterableStorageMap,
    traits::{
        Currency,
        Get,
        ReservableCurrency,
    },
    Parameter,
};
use frame_system::{
    ensure_signed,
    Trait as System,
};
use parity_scale_codec::{
    Codec,
    Decode,
    Encode,
};
use sp_runtime::{
    traits::{
        AtLeast32Bit,
        MaybeSerializeDeserialize,
        Member,
        Zero,
    },
    DispatchResult,
};
use sp_std::{
    fmt::Debug,
    prelude::*,
};

#[derive(PartialEq, Eq, Clone, Encode, Decode, sp_runtime::RuntimeDebug)]
pub struct TreeState<TreeId, AccountId> {
    pub id: TreeId,
    pub parent: Option<TreeId>,
    pub bonded: AccountId,
    pub height: u32,
    pub kids: u32,
    pub size: u32,
}

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as System>::AccountId>>::Balance;
type TreeSt<T> = TreeState<<T as Trait>::TreeId, <T as System>::AccountId>;
pub trait Trait: System {
    /// Overarching event type
    type Event: From<Event<Self>> + Into<<Self as System>::Event>;

    /// The identifier for trees
    type TreeId: Parameter
        + Member
        + AtLeast32Bit
        + Codec
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + PartialOrd
        + PartialEq
        + Zero;

    /// Bond amount, charged per depth
    type Bond: Get<BalanceOf<Self>>;

    /// Maximum group size for all trees
    type MaxSize: Get<u32>;

    /// Maximum depth for all trees
    type MaxDepth: Get<u32>;

    /// Maximum number of subtrees per tree
    type MaxKids: Get<u32>;

    /// Currency type
    type Currency: Currency<Self::AccountId>
        + ReservableCurrency<Self::AccountId>;
}

decl_event!(
    pub enum Event<T>
    where
        <T as Trait>::TreeId,
        <T as System>::AccountId,
        Balance = BalanceOf<T>,
    {
        RegisterIdRoot(TreeId, AccountId, Balance),
        AddedMembers(AccountId, TreeId, Balance),
        RemovedMembers(AccountId, TreeId),
        DelegateBranch(TreeId, TreeId, AccountId, Balance),
        RevokeDelegation(TreeId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        // Tree does not exist
        TreeDNE,
        NotAuthorized,
        CannotAddGroupAboveMaxSize,
        CannotDelegateBelowMaxDepth,
        CannotDelegateAboveMaxKids,
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as Delegate {
        /// The nonce for unique tree id generation
        TreeIdCounter get(fn tree_id_counter): T::TreeId;

        /// The state of trees
        pub Trees get(fn trees): map
            hasher(blake2_128_concat) T::TreeId => Option<TreeSt<T>>;

        /// Membership, also tracks bonded amount for existing members
        pub Members get(fn members): double_map
            hasher(blake2_128_concat) T::TreeId,
            hasher(blake2_128_concat) T::AccountId => Option<BalanceOf<T>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 0]
        fn create_root(
            origin,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let bond = T::Bond::get();
            T::Currency::reserve(&caller, bond)?;
            let id = Self::gen_uid();
            let state = TreeState {
                id,
                parent: None,
                bonded: caller.clone(),
                height: 0u32,
                kids: 0u32,
                size: 1u32,
            };
            <Trees<T>>::insert(id, state);
            <Members<T>>::insert(id, caller.clone(), bond);
            Self::deposit_event(RawEvent::RegisterIdRoot(id, caller, bond));
            Ok(())
        }
        #[weight = 0]
        fn delegate(
            origin,
            parent: T::TreeId,
            members: Vec<T::AccountId>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            ensure!(<Members<T>>::get(parent, &caller).is_some(), Error::<T>::NotAuthorized);
            let parent_st = <Trees<T>>::get(parent).ok_or(Error::<T>::TreeDNE)?;
            let (new_kids, new_height) = (parent_st.kids + 1u32, parent_st.height + 1u32);
            // check that delegating does not violate module kids constraints (num of children)
            ensure!(new_kids <= T::MaxKids::get(), Error::<T>::CannotDelegateAboveMaxKids);
            // check that delegating does not violate module depth constraints
            ensure!(new_height <= T::MaxDepth::get(), Error::<T>::CannotDelegateBelowMaxDepth);
            let bond = Self::reserve_exponential_bond(parent, &caller, new_height, new_kids)?;
            let id = Self::gen_uid();
            let state = TreeState {
                id,
                parent: Some(parent_st.id),
                bonded: caller.clone(),
                height: new_height,
                kids: 0u32,
                size: 0u32,
            };
            Self::add_mems(state, members);
            <Trees<T>>::insert(parent, TreeState {kids: new_kids, ..parent_st});
            Self::deposit_event(RawEvent::DelegateBranch(parent, id, caller, bond));
            Ok(())
        }
        #[weight = 0]
        fn revoke(
            origin,
            branch: T::TreeId,
            penalty: bool,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let tree = <Trees<T>>::get(branch).ok_or(Error::<T>::TreeDNE)?;
            ensure!(tree.bonded == caller, Error::<T>::NotAuthorized);
            Self::remove_mems(tree, None, penalty);
            Self::deposit_event(RawEvent::RevokeDelegation(branch));
            Ok(())
        }
        #[weight = 0]
        fn add_members(
            origin,
            tree_id: T::TreeId,
            members: Vec<T::AccountId>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let tree = <Trees<T>>::get(tree_id).ok_or(Error::<T>::TreeDNE)?;
            // auth requires member of direct parent || bonded caller
            let auth = if let Some(p) = tree.parent {
                <Members<T>>::get(p, &caller).is_some()
            } else { tree.bonded == caller };
            ensure!(auth, Error::<T>::NotAuthorized);
            let mut mems = members; mems.dedup();
            let new_size = mems.len() as u32 + tree.size;
            ensure!(new_size <= T::MaxSize::get(), Error::<T>::CannotAddGroupAboveMaxSize);
            let bond = Self::reserve_linear_bond(tree_id, &caller, new_size)?;
            Self::add_mems(tree, mems);
            Self::deposit_event(RawEvent::AddedMembers(caller, tree_id, bond));
            Ok(())
        }
        #[weight = 0]
        fn remove_members(
            origin,
            tree_id: T::TreeId,
            members: Vec<T::AccountId>,
            penalty: bool,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let tree = <Trees<T>>::get(tree_id).ok_or(Error::<T>::TreeDNE)?;
            // auth requires member of direct parent || bonded caller
            let auth = if let Some(p) = tree.parent {
                <Members<T>>::get(p, &caller).is_some()
            } else { tree.bonded == caller };
            ensure!(auth, Error::<T>::NotAuthorized);
            Self::remove_mems(tree, Some(members), penalty);
            Self::deposit_event(RawEvent::RemovedMembers(caller, tree_id));
            Ok(())
        }
    }
}

// Infallible Storage Mutators
// -> check permissions in caller code before calls
impl<T: Trait> Module<T> {
    /// Generate Unique TreeId
    pub fn gen_uid() -> T::TreeId {
        let mut counter = <TreeIdCounter<T>>::get();
        while <Trees<T>>::get(counter).is_some() {
            counter += 1u32.into();
        }
        counter
    }
    /// Linear Bond
    /// -> bond amount scales linearly with number of members in Tree
    pub fn reserve_linear_bond(
        tree: T::TreeId,
        account: &T::AccountId,
        new_size: u32,
    ) -> Result<BalanceOf<T>, DispatchError> {
        let bond: BalanceOf<T> = T::Bond::get() * new_size.into();
        T::Currency::reserve(account, bond)?;
        let b = if let Some(total) = <Members<T>>::get(tree, account) {
            total + bond
        } else {
            bond
        };
        <Members<T>>::insert(tree, account, b);
        Ok(bond)
    }
    /// Exponential Bond
    /// -> bond amount scales exponentially with height and number of kids
    /// ```(bond)^{height} * (bond)^{kids} = (bond)^{height + kids}```
    pub fn reserve_exponential_bond(
        tree: T::TreeId,
        account: &T::AccountId,
        height: u32,
        kids: u32,
    ) -> Result<BalanceOf<T>, DispatchError> {
        let exp = (height + kids) as usize;
        // Exponential closure n ^ exp
        // - no punishment for calling this and not having enough balance is an attack vector
        // -- could match on reservation error and deduct a fee but would cause storage noop
        let power = |n: BalanceOf<T>, exp: usize| {
            vec![n; exp]
                .iter()
                .fold(BalanceOf::<T>::zero() + 1u32.into(), |a, b| a * *b)
        };
        let bond: BalanceOf<T> = power(T::Bond::get(), exp);
        T::Currency::reserve(account, bond)?;
        let b = if let Some(total) = <Members<T>>::get(tree, account) {
            total + bond
        } else {
            bond
        };
        <Members<T>>::insert(tree, account, b);
        Ok(bond)
    }
    /// Add Members to Tree
    pub fn add_mems(mut tree: TreeSt<T>, mut mems: Vec<T::AccountId>) {
        mems.dedup();
        let mut size_increase = 0u32;
        mems.into_iter().for_each(|m| {
            // only insert if profile does not already exist
            if <Members<T>>::get(tree.id, &m).is_none() {
                <Members<T>>::insert(tree.id, m, BalanceOf::<T>::zero());
                size_increase += 1u32;
            }
        });
        // insert actual size increase
        tree.size += size_increase;
        <Trees<T>>::insert(tree.id, tree);
    }
    /// Remove Members of Tree
    pub fn remove_mems(
        mut tree: TreeSt<T>,
        mems: Option<Vec<T::AccountId>>,
        penalty: bool,
    ) {
        let mut size_decrease = 0u32;
        if let Some(mut mem) = mems {
            mem.dedup();
            mem.into_iter().for_each(|m| {
                if let Some(bond) = <Members<T>>::get(tree.id, &m) {
                    // constraint: cannot remove the account who created the hierarchy
                    if tree.bonded != m {
                        T::Currency::unreserve(&m, bond);
                        if penalty {
                            // (could) transfer the bond to some (treasury) account
                            // instead of returning the bond
                            todo!();
                        }
                        <Members<T>>::remove(tree.id, m);
                        size_decrease += 1u32;
                    }
                }
            });
            // insert actual size decrease
            tree.size -= size_decrease;
            <Trees<T>>::insert(tree.id, tree);
        } else {
            <Members<T>>::iter_prefix(tree.id).for_each(|(a, b)| {
                T::Currency::unreserve(&a, b);
                if penalty {
                    // (could) transfer the bond to some (treasury) account
                    // instead of returning the bond
                    todo!();
                }
                <Members<T>>::remove(tree.id, a);
                size_decrease += 1u32;
            });
            // if parent exists, decrement parent kids count
            if let Some(p) = tree.parent {
                if let Some(tp) = <Trees<T>>::get(p) {
                    <Trees<T>>::insert(
                        p,
                        TreeState {
                            kids: tp.kids - 1,
                            ..tp
                        },
                    );
                }
            }
            // Recursively remove all Children
            // runtime recursion bounded by module-level constraints on
            // * delegation depth/height (MaxDepth)
            // * children (subtrees) per tree (MaxKids)
            // * members (accounts) per tree (MaxSize)
            <Trees<T>>::iter().for_each(|(_, child)| {
                if child.parent == Some(tree.id) {
                    Self::remove_mems(child, None, penalty);
                }
            });
        }
    }
}
