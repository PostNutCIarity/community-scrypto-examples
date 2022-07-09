use scrypto::prelude::*;
use crate::user_management::*;
use crate::lending_pool::*;
use crate::collateral_pool::*;
use crate::structs::{Loan};

#[derive(NonFungibleData)]
pub struct ClaimBadge {
    pub claim_amount: Decimal,
}

// TO-DO:
// * Figure out access controls for this component

blueprint! {
    struct Liquidation {
        // Authority badge that allows the liquidation component to liquidate positions.
        liquidation_bonus: Decimal,
        max_repay: Decimal,
        user_management: Option<ComponentAddress>,
        collateral_pool: Option<ComponentAddress>,
        claim_auth: Vault,
        lending_pool: Option<ComponentAddress>,
    }

    impl Liquidation {
        pub fn new() -> ComponentAddress {

            let claim_auth = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("Claim Auth", "Liquidation Authority Badge")
                .initial_supply(1);

            Self {
                liquidation_bonus: dec!("0.05"),
                max_repay: dec!("0.5"),
                user_management: None,
                collateral_pool: None,
                claim_auth: Vault::with_bucket(claim_auth),
                lending_pool: None,
            }
            .instantiate()
            .globalize()
        } 
        
        pub fn set_address(
            &mut self,
            user_management_address: ComponentAddress,
            lending_pool_address: ComponentAddress,
            collateral_pool_address: ComponentAddress
        ) {
            self.user_management.get_or_insert(user_management_address);
            self.lending_pool.get_or_insert(lending_pool_address);
            self.collateral_pool.get_or_insert(collateral_pool_address);
        }

        // When you take someone's loan make sure they can't just take a random loan NFT.
        // Ways to do this is check the NFT data
        // What kind of permissions do you need to liquidate?
        pub fn liquidate(&mut self, loan_id: NonFungibleId, token_address: ResourceAddress, repay_amount: Bucket) -> Bucket {
            
            // Check to  make sure that the loan can be liquidated
            let lending_pool: LendingPool = self.lending_pool.unwrap().into();
            let get_loan_resource = lending_pool.get_loan_resource();
            let resource_manager = borrow_resource_manager!(get_loan_resource);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            let bad_loans = lending_pool.bad_loans();
            
            assert!(bad_loans.contains(&loan_id) == true, "This loan cannot be liquidated.");

            // Take collateral and liquidate
            let collateral_pool: CollateralPool = self.collateral_pool.unwrap().into();
            let user_id = loan_data.owner;

            // Calculate amount returned
            assert!(repay_amount.amount() <= loan_data.remaining_balance * self.max_repay, "Max repay amount exceeded.");

            let eq_xrd_collateral = repay_amount.amount() / lending_pool.retrieve_xrd_price();

            assert_eq!(repay_amount.amount(), eq_xrd_collateral, "Incorrect calculation");

            let xrd_taken = eq_xrd_collateral + ( eq_xrd_collateral * self.liquidation_bonus );

            let claim_liquidation: Bucket = collateral_pool.redeem(user_id.clone(), token_address, xrd_taken);

            // Update User State
            let user_management: UserManagement = self.user_management.unwrap().into();
            user_management.inc_default(user_id.clone());

            // Update loan
            lending_pool.default_loan(loan_id);

            lending_pool.deposit(user_id, token_address, repay_amount);

            return claim_liquidation
        }

        pub fn liquidatev2(&mut self, loan_id: NonFungibleId)
        {
            // Retrieves lending pool component
            let lending_pool: LendingPool = self.lending_pool.unwrap().into();
            // Retrieves loan NFT resource address
            let get_loan_resource = lending_pool.get_loan_resource();
            // Borrows resource manager to get NFT data
            let resource_manager = borrow_resource_manager!(get_loan_resource);
            // Retreieves loan NFT data
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            // Asserts that the loan liquidation price is less than or equal to the current price of XRD
            assert!(!(loan_data.liquidation_price <= lending_pool.retrieve_xrd_price()), "Can't liquidate this loan since the price of XRD is >= liquidation price");

            // Retreives collateral pool component
            let collateral_pool: CollateralPool = self.collateral_pool.unwrap().into();
            //

        }
    }
}