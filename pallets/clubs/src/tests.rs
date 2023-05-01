use crate::{
	mock,
	mock::{ExtBuilder, System, Test, *},
	pallet::Clubs,
	Error, Event, Pallet,
};

use frame_support::{
	assert_noop, assert_ok, assert_storage_noop, dispatch::RawOrigin, error::BadOrigin,
	traits::Currency,
};
use sp_runtime::{traits::BlockNumberProvider, BoundedVec, DispatchError, ModuleError};

type Balances = <Test as crate::Config>::Currency;
type Module = Pallet<Test>;

mod create_club {
	use super::*;

	#[test]
	fn happy_path() {
		ExtBuilder::default().build_and_execute(|| {
			// Go past genesis block to make sure we can check deposited events.
			System::set_block_number(1);

			let owner = Bob::get();
			let name: BoundedVec<u8, <Test as crate::Config>::MaxNameLength> =
				[0_u8, 1].to_vec().try_into().unwrap();

			assert_ok!(Module::create_club(
				RawOrigin::Signed(Alice::get()).into(),
				name.clone(),
				owner.clone(),
			));

			assert_eq!(Clubs::<Test>::count(), 1);
			let club = Module::clubs(DEFAULT_CLUB_ID).unwrap();
			assert_eq!(club.owner, owner.clone());
			assert_eq!(club.annual_fee, Balance::default());
			assert_eq!(club.name, name);

			System::assert_last_event(mock::RuntimeEvent::Clubs(Event::ClubCreated {
				id: DEFAULT_CLUB_ID,
				owner,
			}));
		});
	}

	#[test]
	fn bad_origin() {
		ExtBuilder::default().build_and_execute(|| {
			assert_noop!(
				Module::create_club(
					RawOrigin::Signed(Bob::get()).into(),
					BoundedVec::default(),
					Alice::get(),
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn balance_issues() {
		ExtBuilder::default().build_and_execute(|| {
			let alice = Alice::get();

			// We have just enough balance to create a club, but we also need the ED.
			Balances::make_free_balance_be(&alice.into(), ClubCreationFee::get());
			assert_storage_noop!(assert!(matches!(
				Module::create_club(
					RawOrigin::Signed(Alice::get()).into(),
					BoundedVec::default(),
					Alice::get(),
				),
				Err(DispatchError::Module(ModuleError { message, .. })) if message == Some("KeepAlive")
			)));

			// Not enough funds to create a club.
			Balances::make_free_balance_be(&alice.into(), Balances::minimum_balance());
			assert_storage_noop!(assert!(matches!(
				Module::create_club(
					RawOrigin::Signed(Alice::get()).into(),
					BoundedVec::default(),
					Alice::get(),
				),
				Err(DispatchError::Module(ModuleError { message, .. }))
					if message == Some("InsufficientBalance")
			)));
		});
	}
}

mod add_member {
	use super::*;

	#[test]
	fn happy_path() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			// Go past genesis block to make sure we can check deposited events.
			System::set_block_number(1);
			let owner = Bob::get();
			let member_id = Dave::get();

			assert_ok!(Module::add_member(
				RawOrigin::Signed(owner).into(),
				DEFAULT_CLUB_ID,
				member_id.clone()
			));

			let member = Module::members(DEFAULT_CLUB_ID, member_id.clone()).unwrap();
			assert_eq!(member.expires_at, 0);

			System::assert_last_event(mock::RuntimeEvent::Clubs(Event::MemberAdded {
				id: DEFAULT_CLUB_ID,
				member_id,
			}));
		});
	}

	#[test]
	fn bad_origin() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let member_id = Dave::get();

			assert_noop!(
				Module::add_member(RawOrigin::None.into(), DEFAULT_CLUB_ID, member_id.clone()),
				BadOrigin
			);
		});
	}

	#[test]
	fn no_club() {
		ExtBuilder::default().build_and_execute(|| {
			let owner = Bob::get();
			let member_id = Dave::get();

			assert_noop!(
				Module::add_member(
					RawOrigin::Signed(owner).into(),
					DEFAULT_CLUB_ID,
					member_id.clone()
				),
				Error::<Test>::NotFound
			);
		});
	}

	#[test]
	fn not_an_owner() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let owner = Dave::get();
			let member_id = Dave::get();

			assert_noop!(
				Module::add_member(
					RawOrigin::Signed(owner).into(),
					DEFAULT_CLUB_ID,
					member_id.clone()
				),
				Error::<Test>::NoPermission
			);
		});
	}

	#[test]
	fn member_already_exists() {
		ExtBuilder::default().with_default_member().build_and_execute(|| {
			let owner = Bob::get();
			let member_id = Dave::get();

			assert_noop!(
				Module::add_member(
					RawOrigin::Signed(owner).into(),
					DEFAULT_CLUB_ID,
					member_id.clone()
				),
				Error::<Test>::AlreadyExists
			);
		});
	}
}

mod extend_membership {
	use super::*;
	use sp_runtime::SaturatedConversion;

	#[test]
	fn happy_path() {
		ExtBuilder::default().with_default_member().build_and_execute(|| {
			// Go past genesis block to make sure we can check deposited events.
			System::set_block_number(1);

			let member_id = Dave::get();
			let years = MaxSubscriptionLength::get();
			assert_ok!(Module::extend_membership(
				RawOrigin::Signed(member_id.clone()).into(),
				DEFAULT_CLUB_ID,
				years
			));

			let member = Module::members(DEFAULT_CLUB_ID, member_id.clone()).unwrap();
			let expires_at = System::current_block_number()
				.saturating_add(BlocksPerYear::get().saturating_mul(years.saturated_into()));

			assert_eq!(member.expires_at, expires_at);

			System::assert_last_event(mock::RuntimeEvent::Clubs(Event::MembershipExtended {
				id: DEFAULT_CLUB_ID,
				member_id,
				expires_at,
			}));
		});
	}

	#[test]
	fn bad_origin() {
		ExtBuilder::default().with_default_member().build_and_execute(|| {
			assert_noop!(
				Module::extend_membership(RawOrigin::None.into(), DEFAULT_CLUB_ID, 100),
				BadOrigin
			);
		});
	}

	#[test]
	fn sub_arg_too_long() {
		ExtBuilder::default().with_default_member().build_and_execute(|| {
			let member_id = Dave::get();
			assert_noop!(
				Module::extend_membership(
					RawOrigin::Signed(member_id.clone()).into(),
					DEFAULT_CLUB_ID,
					MaxSubscriptionLength::get() + 1
				),
				Error::<Test>::SubscriptionTooLong
			);
		});
	}

	#[test]
	fn member_not_found() {
		ExtBuilder::default().build_and_execute(|| {
			let member_id = Dave::get();
			assert_noop!(
				Module::extend_membership(
					RawOrigin::Signed(member_id.clone()).into(),
					DEFAULT_CLUB_ID,
					MaxSubscriptionLength::get()
				),
				Error::<Test>::NotFound
			);
		});
	}
	#[test]
	fn cumulative_sub_too_long() {
		ExtBuilder::default().with_default_member().build_and_execute(|| {
			let member_id = Dave::get();
			assert_ok!(Module::extend_membership(
				RawOrigin::Signed(member_id.clone()).into(),
				DEFAULT_CLUB_ID,
				MaxSubscriptionLength::get()
			));
			assert_noop!(
				Module::extend_membership(
					RawOrigin::Signed(member_id.clone()).into(),
					DEFAULT_CLUB_ID,
					1
				),
				Error::<Test>::SubscriptionTooLong
			);
		});
	}

	#[test]
	fn balance_issues() {
		ExtBuilder::default()
			.with_default_member()
			.with_annual_fee()
			.build_and_execute(|| {
				let member_id = Dave::get();
				let annual_fee = Module::clubs(DEFAULT_CLUB_ID).unwrap().annual_fee;
				Balances::make_free_balance_be(&member_id.into(), annual_fee);

				// We have just enough balance to extend the subscription, but we also need the ED.
				assert_storage_noop!(assert!(matches!(
					Module::extend_membership(
						RawOrigin::Signed(member_id.clone()).into(),
						DEFAULT_CLUB_ID,
						1
					),
					Err(DispatchError::Module(ModuleError { message, .. })) if message == Some("KeepAlive")
				)));

				// Not enough funds to extend a membership.
				assert_storage_noop!(assert!(matches!(
					Module::extend_membership(
						RawOrigin::Signed(member_id.clone()).into(),
						DEFAULT_CLUB_ID,
						2
					),
					Err(DispatchError::Module(ModuleError { message, .. }))
						if message == Some("InsufficientBalance")
				)));
			});
	}
}

mod transfer_ownership {
	use super::*;
	#[test]
	fn happy_path() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			// Go past genesis block to make sure we can check deposited events.
			System::set_block_number(1);

			let owner_id = Bob::get();
			let new_owner_id = Dave::get();
			assert_ok!(Module::transfer_ownership(
				RawOrigin::Signed(owner_id).into(),
				DEFAULT_CLUB_ID,
				new_owner_id.clone()
			));

			let club = Module::clubs(DEFAULT_CLUB_ID).unwrap();
			assert_eq!(club.owner, new_owner_id.clone());

			System::assert_last_event(mock::RuntimeEvent::Clubs(Event::OwnershipTransferred {
				id: DEFAULT_CLUB_ID,
				owner: new_owner_id,
			}));
		});
	}

	#[test]
	fn bad_origin() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let new_owner_id = Dave::get();
			assert_noop!(
				Module::transfer_ownership(
					RawOrigin::None.into(),
					DEFAULT_CLUB_ID,
					new_owner_id.clone()
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn same_owner() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let owner_id = Bob::get();
			assert_noop!(
				Module::transfer_ownership(
					RawOrigin::Signed(owner_id.clone()).into(),
					DEFAULT_CLUB_ID,
					owner_id.clone()
				),
				Error::<Test>::SameOwner
			);
		});
	}

	#[test]
	fn no_club() {
		ExtBuilder::default().build_and_execute(|| {
			let owner_id = Bob::get();
			let new_owner_id = Dave::get();
			assert_noop!(
				Module::transfer_ownership(
					RawOrigin::Signed(owner_id).into(),
					DEFAULT_CLUB_ID,
					new_owner_id,
				),
				Error::<Test>::NotFound
			);
		});
	}

	#[test]
	fn not_an_owner() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let owner_id = Dave::get();
			let new_owner_id = Bob::get();
			assert_noop!(
				Module::transfer_ownership(
					RawOrigin::Signed(owner_id).into(),
					DEFAULT_CLUB_ID,
					new_owner_id.clone()
				),
				Error::<Test>::NoPermission
			);
		});
	}
}

mod set_annual_fee {
	use super::*;

	#[test]
	fn happy_path() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			// Go past genesis block to make sure we can check deposited events.
			System::set_block_number(1);

			let annual_fee = 100;

			let owner_id = Bob::get();
			assert_ok!(Module::set_annual_fee(
				RawOrigin::Signed(owner_id).into(),
				DEFAULT_CLUB_ID,
				annual_fee
			));

			let club = Module::clubs(DEFAULT_CLUB_ID).unwrap();
			assert_eq!(club.annual_fee, annual_fee);

			System::assert_last_event(mock::RuntimeEvent::Clubs(Event::AnnualFeeChanged {
				id: DEFAULT_CLUB_ID,
				annual_fee,
			}));
		});
	}

	#[test]
	fn bad_origin() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			assert_noop!(
				Module::transfer_ownership(RawOrigin::None.into(), DEFAULT_CLUB_ID, 0),
				BadOrigin
			);
		});
	}

	#[test]
	fn no_club() {
		ExtBuilder::default().build_and_execute(|| {
			let owner_id = Bob::get();
			assert_noop!(
				Module::set_annual_fee(RawOrigin::Signed(owner_id).into(), DEFAULT_CLUB_ID, 0),
				Error::<Test>::NotFound
			);
		});
	}

	#[test]
	fn not_an_owner() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let owner_id = Dave::get();
			assert_noop!(
				Module::set_annual_fee(RawOrigin::Signed(owner_id).into(), DEFAULT_CLUB_ID, 0),
				Error::<Test>::NoPermission
			);
		});
	}

	#[test]
	fn same_fee() {
		ExtBuilder::default().with_default_club().build_and_execute(|| {
			let owner_id = Bob::get();
			assert_noop!(
				Module::set_annual_fee(RawOrigin::Signed(owner_id).into(), DEFAULT_CLUB_ID, 0),
				Error::<Test>::SameFee
			);
		});
	}
}
