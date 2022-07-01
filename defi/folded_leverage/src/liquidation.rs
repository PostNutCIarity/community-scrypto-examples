use scrypto::prelude::*;

use crate::lending_pool::*;
use crate::collateral_pool::*;


#[derive(NonFungibleData)]
pub struct ClaimBadge {
    pub claim_amount: Decimal,
}

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct Loan {
    asset: ResourceAddress,
    collateral: ResourceAddress,
    principal_loan_amount: Decimal,
    owner: NonFungibleId,
    #[scrypto(mutable)]
    remaining_balance: Decimal,
    collateral_amount: Decimal,
    collateral_ratio: Decimal,
    loan_status: Status,
}

// TO-DO:
// * Figure out access controls for this component

blueprint! {
    struct Liquidation {
        // Authority badge that allows the liquidation component to liquidate positions.
        liquidation_auth: Vault,
        liquidation_resource: ResourceAddress,
        liquidation_vault: Vault,
        user_management: ComponentAddress,
        collateral_pool: ComponentAddress,
        min_collateral_ratio: Decimal,
        claim_auth: Vault,
        claim_badge_address:ResourceAddress,
        lending_pool: Option<ComponentAddress>,
    }

    impl Liquidation {
        pub fn new(collateral_pool_address: ComponentAddress, user_management_address: ComponentAddress, token_address: ResourceAddress, liquidation_auth: Bucket) -> ComponentAddress {

            let claim_auth = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("Claim Auth", "Liquidation Authority Badge")
                .initial_supply(1);

            let claim_badge_address = ResourceBuilder::new_fungible()
                .metadata("Claim Badge", "Badge used to claim the liquidation")
                .mintable(rule!(require(claim_auth.resource_address())), LOCKED)
                .burnable(rule!(require(claim_auth.resource_address())), LOCKED)
                .no_initial_supply();

            Self {
                liquidation_auth: Vault::with_bucket(liquidation_auth),
                liquidation_resource: token_address,
                liquidation_vault: Vault::new(token_address),
                user_management: user_management_address,
                collateral_pool: collateral_pool_address,
                min_collateral_ratio: dec!("1.0"),
                claim_auth: Vault::with_bucket(claim_auth),
                claim_badge_address: claim_badge_address,
                lending_pool: None,
            }
            .instantiate()
            .globalize()
        }

        pub fn set_lending_address(&mut self, pool_address: ComponentAddress) {
            self.lending_pool.get_or_insert(pool_address);
        }        

        // Think whether you need authorization here
        pub fn view_bad_loans(&self) -> String {
            let lending_pool: LendingPool = self.lending_pool.unwrap().into();
            return lending_pool.get_bad_loans();
        }

        // When you take someone's loan make sure they can't just take a random loan NFT.
        // Ways to do this is check the NFT data
        // What kind of permissions do you need to liquidate?
        pub fn liquidate(&mut self, loan_id: NonFungibleId, token_address: ResourceAddress, liquidate_amount: Decimal) -> Bucket {
            
            // Check to  make sure that the loan can be liquidated
            let lending_pool: LendingPool = self.lending_pool.unwrap().into();
            let get_loan_resource = lending_pool.get_loan_resource();
            let resource_manager = borrow_resource_manager!(get_loan_resource);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            assert!(lending_pool.check_collateral_ratio(loan_id.clone()) == true, "This loan cannot be liquidated.");

            // Take collateral and liquidate
            let collateral_pool: CollateralPool = self.collateral_pool.into();
            let user_id = loan_data.owner;
            let liquidation: Bucket = collateral_pool.redeem(user_id, token_address, liquidate_amount);
            self.liquidation_vault.take(liquidation.amount());
            self.liquidation_auth.authorize(|| liquidation);
            
            // Take Loan NFT and burn

            // Update User State

            let claim_badge: Bucket = self.claim_auth.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.claim_badge_address);
                resource_manager.mint_non_fungible(
                    // The User id
                    &NonFungibleId::random(),
                    // The User data
                    ClaimBadge {
                        claim_amount: liquidate_amount,
                    },
                )
            });

            claim_badge
        }

        pub fn claim(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, claim_badge: Bucket, claim_amount: Decimal) -> Bucket {
            let claim: ClaimBadge = claim_badge.non_fungible().data();
            assert_eq!(claim.claim_amount, claim_badge.amount(), 
            "Claim request must be the same as the amount you liquidated.");
            assert!(claim_badge.resource_address() == self.claim_badge_address, 
            "Claim badge must belong to this protocol.");
            let return_claim: Bucket = self.liquidation_vault.take(claim_amount);
            return_claim
        }
    }
}