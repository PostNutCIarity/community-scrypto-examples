use scrypto::prelude::*;
use crate::lending_pool::*;
use crate::user_management::*;


#[derive(NonFungibleData)]
pub struct LoanDue {
    pub amount_due: Decimal,
}

#[derive(NonFungibleData)]
pub struct AccessBadge {
    pub description: String,
}

// I took the Auto-Lend blueprint example from official Scrypto examples and added a transient token to explore concepts around "folded leverage"
// This would allow user to:
// 1. Deposit collateral to borrow
// 2. Borrow monies
// 3. Deposit borrowed monies
// 4. Borrow additional monies with added borrowed collateral
// 5. Repeat until desired leverage
// 6. Repay loans

// Currently, it's kinda basic and this is a working prototype. But I want to explore additional use-case with this concept.

// This is a barebone implementation of Lending protocol.
//
// Following features are missing:
// * Fees
// * Multi-collateral with price oracle
// * Variable interest rate
// * Authorization
// * Interest dynamic adjustment strategy
// * Upgradability

// TO-DO:
// * Build a design for flash-loan

blueprint! {
    struct FoldedLeverage {

        lending_pools: HashMap<ResourceAddress, LendingPool>,
        //Flash loan
        flash_loan_auth_vault: Vault,
        flash_loan_resource_address: ResourceAddress,
        // Vault that holds the authorization badge
        user_management_address: ComponentAddress,
        // Access badge to allow lending pool component to call a method from user management component. Folded Leverage component does not receive one.
        access_badge: Vault,
        access_badge_token_address: ResourceAddress,
    }

    impl FoldedLeverage {
        /// Creates a lending pool, with single collateral.
        pub fn new() -> ComponentAddress {

            // Creates badge for UserManagement
            let access_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Access Badge")
                .initial_supply(1);   
                
            let access_badge_token = ResourceBuilder::new_fungible()
                .metadata("name", "Access Badge")
                .mintable(rule!(require(access_badge.resource_address())), LOCKED)
                .burnable(rule!(require(access_badge.resource_address())), LOCKED)
                .initial_supply(1);

            let access_badge_token_address = access_badge_token.resource_address();

            // Creates badge to authorizie to mint/burn flash loan
            let flash_loan_token = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Admin authority for BasicFlashLoan")
                .initial_supply(1);

            // Define a "transient" resource which can never be deposited once created, only burned
            let flash_loan_resource_address = ResourceBuilder::new_non_fungible()
                .metadata(
                    "name",
                    "Promise token for BasicFlashLoan - must be returned to be burned!",
                )
                .mintable(rule!(require(flash_loan_token.resource_address())), LOCKED)
                .burnable(rule!(require(flash_loan_token.resource_address())), LOCKED)
                .restrict_deposit(rule!(deny_all), LOCKED)
                .no_initial_supply();
            
            // Difference between using return Self and just Self?
            return Self {
                lending_pools: HashMap::new(),
                flash_loan_auth_vault: Vault::with_bucket(flash_loan_token),
                flash_loan_resource_address: flash_loan_resource_address,
                user_management_address: UserManagement::new(access_badge_token.resource_address(), access_badge_token),
                access_badge: Vault::with_bucket(access_badge),
                access_badge_token_address: access_badge_token_address,
            }
            .instantiate()
            .globalize();

        }

        pub fn new_user(&mut self) -> Bucket {
            let user_management: UserManagement = self.user_management_address.into();
            let new_user: Bucket = user_management.new_user();
            return new_user
        }

        fn get_user(&self, user_auth: &Proof) -> NonFungibleId {
            let user_id = user_auth.non_fungible::<User>().id();
            return user_id
        }

        /// Checks if a liquidity pool for the given pair of tokens exists or not.

        pub fn pool_exists(&self, address: ResourceAddress) -> bool {
            return self.lending_pools.contains_key(&address);
        }

        /// Asserts that a liquidity pool for the given address pair exists
        pub fn assert_pool_exists(&self, address: ResourceAddress, label: String) {
            assert!(
                self.pool_exists(address), 
                "[{}]: No lending pool exists for the given address pair.", 
                label
            );
        }
        
        /// Asserts that a liquidity pool for the given address pair doesn't exist on the DEX.
        
        pub fn assert_pool_doesnt_exists(&self, address: ResourceAddress, label: String) {
            assert!(
                !self.pool_exists(address), 
                "[{}]: A lending pool exists with the given address.", 
                label
            );
        }

        // Need to update user balance 06/01/2022
        // Not sure how to update deposit balance of the account given the transient token mechanic.
        pub fn new_lending_pool(&mut self, user_auth: Proof, deposit: Bucket) {

            let user_management = self.user_management_address.into();

            // Checking if a lending pool already exists for this token
            self.assert_pool_doesnt_exists(
                deposit.resource_address(), 
                String::from("New Liquidity Pool")
            );

            // Checking if user exists
            let user_id = self.get_user(&user_auth);

            let address: ResourceAddress = deposit.resource_address();
            // Sends an access badge to the lending pool
            let access_badge_token = self.access_badge.authorize(|| borrow_resource_manager!(self.access_badge_token_address).mint(Decimal::one()));
            let lending_pool: ComponentAddress = LendingPool::new(user_management, deposit, access_badge_token);
            // Inserts into lending pool hashmap
            self.lending_pools.insert(
                address,
                lending_pool.into()
            );
        }

        pub fn create_transient_token(&mut self) -> Bucket {

            let transient_token = self.flash_loan_auth_vault.authorize(|| {
                borrow_resource_manager!(self.flash_loan_resource_address).mint_non_fungible(
                    &NonFungibleId::random(),
                    LoanDue {
                        amount_due: Decimal::zero(),
                    },
                )
            });

            transient_token
        }

        // Does it/should it matter where the state is updated?
        pub fn deposit(&mut self, user_auth: Proof, token_address: ResourceAddress, amount: Bucket) 
        {
            let address: ResourceAddress = amount.resource_address(); 
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_address, amount.resource_address(), "Token requested and token deposited must be the same.");
            
            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&address);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", address);
                        lending_pool.deposit(user_id, token_address, amount);
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", address);
                    self.new_lending_pool(user_auth, amount)
                }
            }
        }

        // ResourceCheckFailure 06/01/2022
        pub fn borrow(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal) -> Bucket
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Checks collateral ratio (will work at this at some point...)

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        let return_borrow: Bucket = lending_pool.borrow(user_id, token_requested, amount);
                        return_borrow
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[Borrow]: Pool for {:?} doesn't exist.", token_requested);
                    let empty_bucket: Bucket = self.access_badge.take(0);
                    empty_bucket
                }
            }
        }

        // Works but doesn't check lien and doesnt reduce your balance
        pub fn redeem(&mut self, user_auth: Proof, token_reuqested: ResourceAddress, amount: Decimal) -> Bucket {

            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_reuqested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_reuqested);       
                        let return_bucket: Bucket = lending_pool.redeem(user_id, token_reuqested, amount);
                        return_bucket
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_reuqested);
                    let empty_bucket: Bucket = self.access_badge.take(0);
                    empty_bucket
                }
            }
        }

        // Works but doesn't reduce balance 06/01/22
        pub fn repay(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Bucket) -> Bucket {

            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_requested, amount.resource_address(), "Token requested and token deposited must be the same.");

            // Repay fully or partial?
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        let return_bucket: Bucket = lending_pool.repay(user_id, token_requested, amount);
                        return_bucket
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_requested);
                    let empty_bucket: Bucket = self.access_badge.take(0);
                    empty_bucket
                }
            }
        }
    }
}