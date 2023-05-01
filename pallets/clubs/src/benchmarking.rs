//! Benchmarking setup for pallet-clubs

use crate::{
	pallet::Members, BalanceOf, Call, ClubDetails, ClubId, Clubs, Config, Event, MemberDetails,
	Pallet,
};
use frame_benchmarking::{account, v1::benchmarks, BenchmarkError, Vec};
use frame_support::{
	dispatch::{RawOrigin, UnfilteredDispatchable},
	sp_runtime::{SaturatedConversion, Saturating},
	traits::{Currency, EnsureOrigin, Get},
};
use frame_system::ensure_signed;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn fund_account<T: Config>(who: &T::AccountId) {
	T::Currency::deposit_creating(
		&who,
		T::Currency::minimum_balance().saturating_mul(10000_u32.saturated_into()),
	);
}

fn seed_club<T: Config>(club_id: ClubId, owner: &T::AccountId, annual_fee: u8) {
	Clubs::<T>::insert(
		club_id,
		ClubDetails {
			name: Vec::new().try_into().unwrap(),
			owner: owner.clone(),
			annual_fee: annual_fee.saturated_into(),
		},
	)
}

fn seed_member<T: Config>(club_id: ClubId, member_id: &T::AccountId) {
	Members::<T>::insert(club_id, member_id.clone(), MemberDetails::default());
}

benchmarks! {
	create_club {
		let owner: T::AccountId = account("bob", 0, 0);
		let origin = T::RootOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let caller = T::RootOrigin::ensure_origin(origin.clone()).unwrap();
		let call = Call::<T>::create_club {
			name: Vec::from_iter(0_u8..T::MaxNameLength::get() as u8).try_into().unwrap(),
			owner: owner.clone()
		};
		let who = ensure_signed(origin.clone()).map_err(|_| BenchmarkError::Weightless)?;
		let next_id = Clubs::<T>::count().saturating_add(1);
		fund_account::<T>(&who);
	}: {call.dispatch_bypass_filter(origin)?}
	verify {
		assert_last_event::<T>(Event::ClubCreated {id: next_id, owner }.into())
	}

	add_member {
		let owner: T::AccountId = account("bob", 0, 0);
		let club_id: ClubId = 1;
		let member_id: T::AccountId = account("dave", 0, 0);
		seed_club::<T>(club_id, &owner, 10);
	}: _(RawOrigin::Signed(owner), club_id, member_id.clone())
	verify {
		assert_last_event::<T>(Event::MemberAdded {id: club_id, member_id }.into())
	}

	extend_membership {
		let owner: T::AccountId = account("bob", 0, 0);
		let club_id: ClubId = 1;
		let member_id: T::AccountId = account("dave", 0, 0);
		let years = 100;
		seed_club::<T>(club_id, &owner, 10);
		seed_member::<T>(club_id, &member_id);
		fund_account::<T>(&member_id);
	}: _(RawOrigin::Signed(member_id.clone()), club_id, years)
	verify {
		let current_block = frame_system::Pallet::<T>::block_number();
		assert_last_event::<T>(Event::MembershipExtended {
			id: club_id,
			expires_at: T::BlocksPerYear::get()
				.saturating_mul(years.into())
				.saturating_add(current_block),
			member_id
		}.into());
	}

	transfer_ownership {
		let owner: T::AccountId = account("bob", 0, 0);
		let club_id: ClubId = 1;
		let new_owner: T::AccountId = account("dave", 0, 0);
		seed_club::<T>(club_id, &owner, 10);
	}: _(RawOrigin::Signed(owner), club_id, new_owner.clone())
	verify {
		assert_last_event::<T>(Event::OwnershipTransferred {id: club_id, owner: new_owner }.into())
	}

	set_annual_fee {
		let owner: T::AccountId = account("bob", 0, 0);
		let club_id: ClubId = 1;
		let annual_fee: BalanceOf<T> = 100_u8.saturated_into();
		seed_club::<T>(club_id, &owner, 0);
	}: _(RawOrigin::Signed(owner), club_id, annual_fee)
	verify {
		assert_last_event::<T>(Event::AnnualFeeChanged {id: club_id, annual_fee }.into())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
