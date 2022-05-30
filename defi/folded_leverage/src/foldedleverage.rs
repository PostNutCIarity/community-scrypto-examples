use scrypto::prelude::*;
use crate::lending_pool::*;
use crate::user_management::*;

// Refactor to main component
#[derive(NonFungibleData)]
pub struct LoanDue {
    pub amount_due: Decimal,
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

blueprint! {
    struct FoldedLeverage {

        lending_pools: HashMap<ResourceAddress, LendingPool>,
        //Flash loan
        auth_vault: Vault,
        transient_resource_address: ResourceAddress,
        // Vault that holds the authorization badge
        user_management_address: ComponentAddress,
        // Not sure if I should use this yet. Meant to be used to check if user exist as a way to reduce
        // The usage of passing Proof as a parameter since it causes a lot of use of moved values issues.
        user_data: HashMap<NonFungibleId, User>,
    }

    impl FoldedLeverage {
        /// Creates a lending pool, with single collateral.
        pub fn new() -> ComponentAddress {

            // Creates badge to authorizie to mint/burn flash loan
            let auth_token = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Admin authority for BasicFlashLoan")
                .initial_supply(1);

            // Define a "transient" resource which can never be deposited once created, only burned
            let address = ResourceBuilder::new_non_fungible()
                .metadata(
                    "name",
                    "Promise token for BasicFlashLoan - must be returned to be burned!",
                )
                .mintable(rule!(require(auth_token.resource_address())), LOCKED)
                .burnable(rule!(require(auth_token.resource_address())), LOCKED)
                .restrict_deposit(rule!(deny_all), LOCKED)
                .no_initial_supply();
            
            // Difference between using return Self and just Self?
            return Self {
                lending_pools: HashMap::new(),
                auth_vault: Vault::with_bucket(auth_token),
                transient_resource_address: address,
                user_management_address: UserManagement::new(),
                user_data: HashMap::new(),
            }
            .instantiate()
            .globalize()
        }

        // ResourceCheckFailure when calling through here.
        pub fn new_user(&mut self) -> Bucket {
            let user_management: UserManagement = self.user_management_address.into();
            let new_user: Bucket = user_management.new_user();
            let user_id: NonFungibleId = user_management.new_user().non_fungible::<User>().id();
            let user: User = user_management.new_user().non_fungible().data();
            self.user_data.insert(user_id, user);
            return new_user
        }

        fn check_user(&self, user_auth: &Proof) -> bool {
            let user_id = user_auth.non_fungible::<User>().id();
            return self.user_data.contains_key(&user_id);
        }

        fn get_user(&self, user_auth: &Proof) -> NonFungibleId {
            let user_id = user_auth.non_fungible::<User>().id();
            return user_id
        }
        
        fn assert_user_exist(&self, user_auth: &Proof) {
            assert!(self.check_user(&user_auth), "User doesn't exist");
        }

        // Temporary insert user function
        pub fn insert_user(&mut self, user_auth: Proof) {
            let user_id = self.get_user(&user_auth);
            let user: User = user_auth.non_fungible().data();
            self.user_data.insert(user_id, user);
        }

        /// Checks if a liquidity pool for the given pair of tokens exists or not.

        pub fn pool_exists(&self, address: ResourceAddress) -> bool {
            return self.lending_pools.contains_key(&address);
        }

        /// Asserts that a liquidity pool for the given address pair exists on the DEX.

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

        // CantMoveRestrictedProof
        // Temporarily remove Proof for now
        pub fn new_lending_pool(&mut self, deposit: Bucket) {
            // Checking if a lending pool already exists for this token
            self.assert_pool_doesnt_exists(
                deposit.resource_address(), 
                String::from("New Liquidity Pool")
            );

            // Checking if user exists

            let address: ResourceAddress = deposit.resource_address();
            // Proof is pass through here
            let lending_pool: ComponentAddress = LendingPool::new(deposit);
            // Inserts into lending pool hashmap
            self.lending_pools.insert(
                address,
                lending_pool.into()
            );
        }

        //  AuthorizationError { function: "update_non_fungible_mutable_data", authorization: Protected(ProofRule(This(Resource(0331ff6b85e7c537bd1fa3c81db29f632e1a8e5c7e1df1f573dacb)))), error: NotAuthorized }
        pub fn deposit(&mut self, user_auth: Proof, token_address: ResourceAddress, amount: Bucket) 
        {
            let address: ResourceAddress = amount.resource_address(); 
            let user_management: UserManagement = self.user_management_address.into();

            // Checks if the user exists
            self.assert_user_exist(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_address, amount.resource_address(), "Token requested and token deposited must be the same.");
            
            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&address);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", address);

                    let user_id = user_auth.resource_address();
                    // Check if user exist => 
                    match user_management.check_user_exist(user_id) {
                        true => {
                            // Update user state 
                            let user_id = self.get_user(&user_auth);
                            let dec_amount = amount.amount();
                            // Does it not add the balance in the first try?
                            user_management.add_deposit_balance(user_id, token_address, dec_amount);
                            lending_pool.deposit(amount);
                        }
                        false => {
                            println!("User doesn't exist!");
                        }
                    };

                }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", address);
                    self.new_lending_pool(amount)
                }
            }
        }

        pub fn borrow(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal) 
        {
            let user_management: UserManagement = self.user_management_address.into();

            // Checks if user NFT contains resources. If it doesn't add_deposit_balance will add that to the NFT.
            user_management.assert_deposit_resouce_doesnt_exists(user_auth.clone(), token_requested, 
            String::from("Adding resource to account"));

            // Checks if the user exists
            self.assert_user_exist(&user_auth);

            // Checks collateral ratio (will work at this at some point...)

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);

                    let user_id = user_auth.resource_address();
                    // Check if user exist => 
                    match user_management.check_user_exist(user_id) {
                        true => {
                            lending_pool.borrow(user_auth.clone(), token_requested, amount);
                        }
                        false => {
                            println!("User doesn't exist!");
                        }
                    };

                    // Update user state 
                    let user_id = self.get_user(&user_auth);
                    user_management.add_borrow_balance(user_id, token_requested, amount);
                }

                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[Borrow]: Pool for {:?} doesn't exist.", token_requested);
                }
            }
        }

        // Still need to finish
        pub fn redeem(&mut self, user_auth: Proof, token_reuqested: ResourceAddress, amount: Decimal) {

            let user_management: UserManagement = self.user_management_address.into();

            // Check if deposit withdrawal request has no lien ----> Should checks be here or in the liquidity pool component?
            user_management.check_lien(user_auth.clone(), token_reuqested);

            // Checks if the user exists
            self.assert_user_exist(&user_auth);

            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_reuqested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_reuqested);

                    let user_id = user_auth.resource_address();
                    // Check if user exist => 
                    match user_management.check_user_exist(user_id) {
                        true => {
                            lending_pool.redeem(user_auth.clone(), token_reuqested, amount);
                        }
                        false => {
                            println!("User doesn't exist!");
                        }
                    };

                    // Update user state NEED TO FIX!!!
                    let user_id = self.get_user(&user_auth);
                    user_management.add_borrow_balance(user_id, token_reuqested, amount);
                }

                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_reuqested);
                }
            }
        }

        pub fn repay(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Bucket) {

            let user_management: UserManagement = self.user_management_address.into();

            // Checks if the user exists
            self.assert_user_exist(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_requested, amount.resource_address(), "Token requested and token deposited must be the same.");

            // Repay fully or partial?
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                    let user_id = user_auth.resource_address();
                    // Check if user exist => 
                    match user_management.check_user_exist(user_id) {
                        true => {
                            let user_id = self.get_user(&user_auth);
                            let dec_amount = amount.amount();
                            user_management.add_borrow_balance(user_id, token_requested, dec_amount);
                            lending_pool.repay(user_auth, token_requested, amount);
                        }
                        false => {
                            println!("User doesn't exist!");
                        }
                    };

                    // Update user state


                }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_requested);
                }
            }

        }
    }
}