use crate as pallet_clubs;
use crate::{weights::NodeTplWeight, BalanceOf, ClubId, Pallet};
use frame_support::{
	dispatch::RawOrigin,
	ord_parameter_types,
	pallet_prelude::ConstU32,
	traits::{ConstU16, ConstU64},
	BoundedVec,
};
use frame_system::EnsureSignedBy;
use sp_core::{parameter_types, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u64;
pub(crate) type Balance = u128;
pub(crate) type BlockNumber = u64;

pub(crate) const DEFAULT_CLUB_ID: ClubId = 1;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Clubs: pallet_clubs,
		Balances: pallet_balances,
	}
);

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const ClubCreationFee: Balance = 10;
	pub const BlocksPerYear: BlockNumber = 100;
	pub const MaxSubscriptionLength: u16 = 100;
}

ord_parameter_types! {
	pub const Alice: AccountId = 1;
	pub const Bob: AccountId = 2;
	pub const Dave: AccountId = 3;
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ConstU32<1024>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

impl pallet_clubs::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxNameLength = ConstU32<255>;
	type MaxSubscriptionLength = MaxSubscriptionLength;
	type BlocksPerYear = BlocksPerYear;
	type Currency = Balances;
	type ClubCreationFee = ClubCreationFee;
	type RootOrigin = EnsureSignedBy<Alice, AccountId>;
	type WeightInfo = NodeTplWeight<Self>;
}

#[derive(Default)]
pub(crate) struct ExtBuilder {
	default_club: bool,
	default_member: bool,
	annual_fee: BalanceOf<Test>,
}

impl ExtBuilder {
	pub fn with_default_club(mut self) -> Self {
		self.default_club = true;
		self
	}

	pub fn with_default_member(mut self) -> Self {
		self.default_member = true;
		// Can't have a member without a club.
		self.with_default_club()
	}

	pub fn with_annual_fee(mut self) -> Self {
		self.annual_fee = 10;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		let _ = pallet_balances::GenesisConfig::<Test> {
			balances: vec![(Alice::get(), 1000), (Bob::get(), 1000)],
		}
		.assimilate_storage(&mut storage);

		storage.into()
	}

	pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
		let default_club = self.default_club;
		let default_member = self.default_member;
		let annual_fee = self.annual_fee;
		let mut ext = self.build();
		ext.execute_with(|| {
			if default_club {
				let owner = Bob::get();
				let name: BoundedVec<u8, <Test as crate::Config>::MaxNameLength> =
					[0_u8, 1].to_vec().try_into().unwrap();

				Pallet::<Test>::create_club(
					RawOrigin::Signed(Alice::get()).into(),
					name.clone(),
					owner.clone(),
				)
				.unwrap();
			}

			if annual_fee > 0 {
				let owner = Bob::get();

				Pallet::<Test>::set_annual_fee(
					RawOrigin::Signed(owner).into(),
					DEFAULT_CLUB_ID,
					annual_fee,
				)
				.unwrap();
			}

			if default_member {
				let owner = Bob::get();
				let member_id = Dave::get();

				Pallet::<Test>::add_member(
					RawOrigin::Signed(owner).into(),
					DEFAULT_CLUB_ID,
					member_id.clone(),
				)
				.unwrap();
			}
		});
		ext.execute_with(test)
	}
}
