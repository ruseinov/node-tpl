//! # Clubs Pallet
//! A pallet that allows for creating and joining clubs.
//!
//! Allows for specifying optional fees for club creation and membership.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Event`]
//! - [`Error`]

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::*, traits::Currency};
pub use pallet::*;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Used to uniquely identify each club instance.
pub(crate) type ClubId = u32;

/// Club details.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(AccountId, MaxNameLength, Balance))]
pub struct ClubDetails<AccountId, MaxNameLength: Get<u32>, Balance> {
	/// Club name.
	pub name: BoundedVec<u8, MaxNameLength>,
	/// Club owner. Can be transferred to another [`AccountId`].
	pub owner: AccountId,
	/// Annual membership fee.
	pub annual_fee: Balance,
}

/// Club member details.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
#[scale_info(skip_type_params(BlockNumber))]
pub struct MemberDetails<BlockNumber> {
	/// Used to identify active members.
	pub expires_at: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use crate::*;
	use frame_support::{
		defensive,
		sp_runtime::{SaturatedConversion, Saturating},
		traits::{Currency, ExistenceRequirement, WithdrawReasons},
	};
	use frame_system::pallet_prelude::*;
	pub use weights::WeightInfo;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Pallet configuration.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The maximum length of a club name.
		#[pallet::constant]
		type MaxNameLength: Get<u32>;

		/// The maximum subscription length in years.
		#[pallet::constant]
		type MaxSubscriptionLength: Get<u16>;

		/// Approximate number of blocks produced per year in order to calculate subscription
		/// expiration time.
		#[pallet::constant]
		type BlocksPerYear: Get<Self::BlockNumber>;

		/// Currency trait to facilitate fee payments.
		type Currency: Currency<Self::AccountId>;

		/// The cost of introducing a new club.
		#[pallet::constant]
		type ClubCreationFee: Get<BalanceOf<Self>>;

		/// Origin for admin-level operations, like creating a club.
		type RootOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The weight information for this pallet.
		type WeightInfo: WeightInfo;
	}

	/// A map of all existing clubs. Maps [`ClubId`] to [`ClubDetails`]. The id is a pseudo auto
	/// increment based on the map size: `Clubs::<T>::count() + 1`.
	#[pallet::storage]
	#[pallet::getter(fn clubs)]
	pub(crate) type Clubs<T: Config> = CountedStorageMap<
		_,
		Blake2_128Concat,
		ClubId,
		ClubDetails<T::AccountId, T::MaxNameLength, BalanceOf<T>>,
		OptionQuery,
	>;

	/// A double map of club members. Maps [`ClubId`] to [`AccountId`] to [`MemberDetails`].
	#[pallet::storage]
	#[pallet::getter(fn members)]
	pub(crate) type Members<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		ClubId,
		Blake2_128Concat,
		T::AccountId,
		MemberDetails<T::BlockNumber>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A club has been created successfully.
		ClubCreated { id: ClubId, owner: T::AccountId },

		/// Not enough founds to perform an action.
		InsufficientFunds,

		/// A member has been added to a club.
		MemberAdded { id: ClubId, member_id: T::AccountId },

		/// A membership has been extended.
		MembershipExtended { id: ClubId, member_id: T::AccountId, expires_at: T::BlockNumber },

		/// A club has been transferred to another owner.
		OwnershipTransferred { id: ClubId, owner: T::AccountId },

		/// Club's annual fee has been changed.
		AnnualFeeChanged { id: ClubId, annual_fee: BalanceOf<T> },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// [`ClubId`] has reached it's maximum value. Consider changing it's type to something
		/// bigger.
		ClubIdOverflow,

		/// User does not have a permission to perform an action.
		NoPermission,

		/// Storage item not found.
		NotFound,

		/// Storage item already exists.
		AlreadyExists,

		/// A user's subscription at any given time can't be longer than
		/// [`Config::MaxSubscriptionLength`].
		SubscriptionTooLong,

		/// The owner_id specified for ownership transfer is the same as the current one.
		SameOwner,

		/// The annual fee specified is the same as it was previously.
		SameFee,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a club.
		///
		/// Origin must be signed by [`Config::Root`].
		///
		/// Arguments:
		/// - `name`: The club name.
		/// - `owner`: An account of the club owner.
		/// - `annual_fee`: Annual membership fee.
		///
		/// Emits [`Event::ClubCreated`].
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::create_club())]
		pub fn create_club(
			origin: OriginFor<T>,
			name: BoundedVec<u8, T::MaxNameLength>,
			owner: T::AccountId,
		) -> DispatchResult {
			T::RootOrigin::ensure_origin(origin.clone())?;
			let who = ensure_signed(origin)?;

			let next_id = Clubs::<T>::count().saturating_add(1);
			// A nice to have, but hard to test. Ideally should be done through storage_alias faking
			// the counter, but not worth the hassle, just like testing defensive errors.
			ensure!(Self::clubs(next_id).is_none(), Error::<T>::ClubIdOverflow);

			// We are dropping the imbalance for simplicity, which decreases total issuance. There
			// are plenty of options on how to deal with this, including sending it to treasury.
			let _ = T::Currency::withdraw(
				&who,
				T::ClubCreationFee::get(),
				WithdrawReasons::FEE,
				ExistenceRequirement::KeepAlive,
			)?;

			let club =
				ClubDetails { name, owner: owner.clone(), annual_fee: 0_u8.saturated_into() };

			Clubs::<T>::insert(next_id, club);

			Self::deposit_event(Event::ClubCreated { id: next_id, owner });

			Ok(())
		}

		/// Adds a club member.
		///
		/// Origin must be signed by club owner.
		///
		/// Arguments:
		/// - `club_id`: A unique club identifier.
		/// - `member_id`: An account of a new club member.
		///
		/// Emits [`Event::MemberAdded`].
		///
		/// A club owner can also be a club member if they wish to formalize their subscription
		/// payments. That also makes things easier when one wants to transfer ownership, but keep
		/// being a member of the club. That also makes bookkeeping easier in case ownership is
		/// transferred to a club member with an active subscription.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::add_member())]
		pub fn add_member(
			origin: OriginFor<T>,
			club_id: ClubId,
			member_id: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let club = Self::clubs(club_id);

			ensure!(club.is_some(), Error::<T>::NotFound);

			if let Some(club) = club {
				ensure!(club.owner == who, Error::<T>::NoPermission);

				ensure!(
					Self::members(club_id, member_id.clone()).is_none(),
					Error::<T>::AlreadyExists
				);

				Members::<T>::insert(club_id, member_id.clone(), MemberDetails::default());

				Self::deposit_event(Event::<T>::MemberAdded { id: club_id, member_id });
			}

			Ok(())
		}

		/// Extend membership by some years.
		///
		/// The origin has to be signed by a club member that wishes to extend their membership.
		///
		/// Arguments:
		/// - `club_id`: A unique club identifier.
		/// - `years`: A number of years a member wishes to extend their membership for.
		///
		/// Emits [`Event::MembershipExtended`].
		///
		/// In case the fee is 0 - a user only pays transaction fees.
		/// A subscription cannot be longer than a 100 years.
		/// In case the subscription has already expired - we start a new one from the current
		/// block.
		/// [`Config::BlockNumber`].
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::extend_membership())]
		pub fn extend_membership(
			origin: OriginFor<T>,
			club_id: ClubId,
			years: u16,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(years <= T::MaxSubscriptionLength::get(), Error::<T>::SubscriptionTooLong);

			let member = Self::members(club_id, who.clone());

			ensure!(member.is_some(), Error::<T>::NotFound);

			if let Some(details) = member {
				let mut expires_at = details.expires_at;
				let current_block = frame_system::Pallet::<T>::block_number();
				let blocks_per_year = T::BlocksPerYear::get();

				// Make sure we align the base block for subscription renewal in case of expiration.
				if expires_at < current_block {
					expires_at = current_block;
				} else {
					let current_len = expires_at.saturating_sub(current_block) / blocks_per_year;
					ensure!(
						current_len + years.into() <= T::MaxSubscriptionLength::get().into(),
						Error::<T>::SubscriptionTooLong
					);
				}
				expires_at =
					expires_at.saturating_add(blocks_per_year.saturating_mul(years.into()));

				let club = Self::clubs(club_id);
				if let Some(club_details) = club {
					// We are dropping the imbalance for simplicity, which decreases total
					// issuance. There are plenty of options on how to deal with this, including
					// sending it to treasury.
					let _ = T::Currency::withdraw(
						&who,
						club_details.annual_fee * years.into(),
						WithdrawReasons::FEE,
						ExistenceRequirement::KeepAlive,
					)?;
				} else {
					defensive!("Club exists; qed");
				}

				Members::<T>::insert(club_id, who.clone(), MemberDetails { expires_at });

				Self::deposit_event(Event::<T>::MembershipExtended {
					id: club_id,
					member_id: who,
					expires_at,
				});
			} else {
				defensive!("Member exists; qed");
			}

			Ok(())
		}

		/// Transfers club ownership.
		///
		/// Origin must be signed by club owner.
		///
		/// Arguments:
		/// - `club_id`: A unique club identifier.
		/// - `owner_id`: An account of a new club owner.
		///
		/// Emits [`Event::OwnershipTransferred`].
		///
		/// If the new owner has an active subscription - it's going to be kept intact.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::transfer_ownership())]
		pub fn transfer_ownership(
			origin: OriginFor<T>,
			club_id: ClubId,
			owner_id: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(who != owner_id, Error::<T>::SameOwner);
			let club = Self::clubs(club_id);

			ensure!(club.is_some(), Error::<T>::NotFound);

			if let Some(club) = club {
				ensure!(club.owner == who, Error::<T>::NoPermission);

				Clubs::<T>::mutate(club_id, |c| {
					if let Some(ref mut club_details) = c {
						club_details.owner = owner_id.clone();
					}
				});

				Self::deposit_event(Event::<T>::OwnershipTransferred {
					id: club_id,
					owner: owner_id,
				});
			}

			Ok(())
		}

		/// Sets club's annual fee.
		///
		/// Origin must be signed by club owner.
		///
		/// Arguments:
		/// - `club_id`: A unique club identifier.
		/// - `annual_fee`: An amount to be charged for membership annually.
		///
		/// Emits [`Event::AnnualFeeChanged`].
		///
		/// Does not affect any previously paid memberships.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::set_annual_fee())]
		pub fn set_annual_fee(
			origin: OriginFor<T>,
			club_id: ClubId,
			annual_fee: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let club = Self::clubs(club_id);

			ensure!(club.is_some(), Error::<T>::NotFound);

			if let Some(club) = club {
				ensure!(club.owner == who, Error::<T>::NoPermission);
				ensure!(club.annual_fee != annual_fee, Error::<T>::SameFee);

				Clubs::<T>::mutate(club_id, |c| {
					if let Some(ref mut club_details) = c {
						club_details.annual_fee = annual_fee
					}
				});

				Self::deposit_event(Event::<T>::AnnualFeeChanged { id: club_id, annual_fee });
			} else {
				defensive!("Club exists; qed");
			}

			Ok(())
		}
	}
}
