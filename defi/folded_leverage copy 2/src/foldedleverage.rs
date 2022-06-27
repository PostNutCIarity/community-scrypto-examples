use scrypto::prelude::*;
use crate::lending_pool::*;
use crate::collateral_pool::*;
use crate::user_management::*;
use crate::user_management::User;


#[derive(NonFungibleData, Debug)]
pub struct FlashLoan {
    pub amount_due: Decimal,
    pub borrow_count: u8,
}

#[derive(NonFungibleData)]
pub struct AccessBadge {
    pub description: String,
}

// TO-DO:
// * Build a design for flash-loan
// * See why vault can't be empty
// * Naming/identifying each pool
// * Add LP token calculation between converting deposits to collateral
// * There is some complications when you convert deposit to collateral as it relates to LP tokens
// * Delineate between user management and the loan nfts

blueprint! {
    struct FoldedLeverage {

        lending_pools: HashMap<ResourceAddress, LendingPool>,
        collateral_pools: HashMap<ResourceAddress, CollateralPool>,
        collateral_pool_address: HashMap<ResourceAddress, ComponentAddress>,
        //Flash loan
        flash_loan_auth_vault: Vault,
        flash_loan_resource_address: ResourceAddress,
        // Vault that holds the authorization badge
        user_management_address: ComponentAddress,
        // Access badge to allow lending pool component to call a method from user management component. Folded Leverage component does not receive one.
        access_vault: Vault,
        access_badge_token_vault: Vault,
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
                
            let mut access_badge_token = ResourceBuilder::new_fungible()
                .metadata("name", "Access Badge")
                .mintable(rule!(require(access_badge.resource_address())), LOCKED)
                .burnable(rule!(require(access_badge.resource_address())), LOCKED)
                .initial_supply(2);

            let access_badge_token_address = access_badge_token.resource_address();
            let access_badge_token_user: Bucket = access_badge_token.take(1);

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
                .updateable_non_fungible_data(rule!(require(flash_loan_token.resource_address())), LOCKED)
                .restrict_deposit(rule!(deny_all), LOCKED)
                .no_initial_supply();

            
            // Difference between using return Self and just Self?
            return Self {
                lending_pools: HashMap::new(),
                collateral_pools: HashMap::new(),
                collateral_pool_address: HashMap::new(),
                flash_loan_auth_vault: Vault::with_bucket(flash_loan_token),
                flash_loan_resource_address: flash_loan_resource_address,
                user_management_address: UserManagement::new(access_badge_token.resource_address(), access_badge_token_user),
                access_vault: Vault::with_bucket(access_badge),
                access_badge_token_vault: Vault::with_bucket(access_badge_token),
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

        /// Need to update user balance 06/01/2022
        /// Not sure how to update deposit balance of the account given the transient token mechanic.
        /// Updated so the balance will update, but have to think about the design further whether it makes sense 06/02/22
        pub fn new_lending_pool(&mut self, user_auth: Proof, token_address: ResourceAddress, deposit: Bucket) -> Bucket {

            let user_management = self.user_management_address.into();

            // Checking if a lending pool already exists for this token
            self.assert_pool_doesnt_exists(
                deposit.resource_address(), 
                String::from("New Liquidity Pool")
            );

            // Checking if user exists
            let user_id = self.get_user(&user_auth);

            let deposit_amount = deposit.amount();

            let address: ResourceAddress = deposit.resource_address();
            // Sends an access badge to the lending pool
            let access_badge_token = self.access_vault.authorize(|| borrow_resource_manager!(self.access_badge_token_address).mint(Decimal::one()));
            
            let (lending_pool, transient_token, lp_tokens): (ComponentAddress, Bucket, Bucket) = LendingPool::new(user_management, deposit, access_badge_token);

            let user_management: UserManagement = self.user_management_address.into();
            // FoldedLeverage component registers the transient token is this bad? 06/02/22
            // Is FoldedLeverage component even allowed to register resource?
            let transient_token_address = transient_token.resource_address();
            self.access_badge_token_vault.authorize(|| {user_management.register_resource(transient_token_address)});
            user_management.add_deposit_balance(user_id, token_address, deposit_amount, transient_token);

            // Inserts into lending pool hashmap
            self.lending_pools.insert(
                address,
                lending_pool.into()
            );

            return lp_tokens

        }

        pub fn new_collateral_pool(&mut self, user_auth: Proof, token_address: ResourceAddress, collateral: Bucket) {

            let user_management = self.user_management_address.into();

            // Checking if a lending pool already exists for this token
            // self.assert_pool_doesnt_exists(
                // collateral.resource_address(), 
                // String::from("New Collateral Pool")
            // );

            // Checking if user exists
            let user_id = self.get_user(&user_auth);

            let deposit_amount = collateral.amount();

            let address: ResourceAddress = collateral.resource_address();
            // Sends an access badge to the collateral pool
            let access_badge_token = self.access_vault.authorize(|| borrow_resource_manager!(self.access_badge_token_address).mint(Decimal::one()));

            let (collateral_pool, transient_token): (ComponentAddress, Bucket) = CollateralPool::new(user_management, collateral, access_badge_token);

            let user_management: UserManagement = self.user_management_address.into();
            // FoldedLeverage component registers the transient token is this bad? 06/02/22
            // Is FoldedLeverage component even allowed to register resource?
            let transient_token_address = transient_token.resource_address();
            self.access_badge_token_vault.authorize(|| {user_management.register_resource(transient_token_address)});
            user_management.add_deposit_balance(user_id, token_address, deposit_amount, transient_token);

            // Inserts into lending pool hashmap
            self.collateral_pools.insert(
                address,
                collateral_pool.into()
            );

            self.collateral_pool_address.insert(
                address,
                collateral_pool.into()
            );

        }

        pub fn deposit_supply(&mut self, user_auth: Proof, token_address: ResourceAddress, amount: Bucket) -> Bucket  
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
                        let lp_tokens: Bucket = lending_pool.deposit(user_id, token_address, amount);
                        lp_tokens
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", address);
                    self.new_lending_pool(user_auth, token_address, amount)
                }
            }
        }

        pub fn deposit_collateral(&mut self, user_auth: Proof, token_address: ResourceAddress, amount: Bucket)  
        {
            let address: ResourceAddress = amount.resource_address(); 
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_address, amount.resource_address(), "Token requested and token deposited must be the same.");
            
            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_collateral_pool: Option<&CollateralPool> = self.collateral_pools.get(&address);
            match optional_collateral_pool {
                Some (collateral_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", address);
                        collateral_pool.deposit(user_id, token_address, amount);
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", address);
                    self.new_collateral_pool(user_auth, token_address, amount)
                }
            }
        }

        // Converts the deposit supply to a collateral supply
        pub fn convert_to_collateral(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal)
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        lending_pool.convert_to_collateral(user_id, token_requested, amount);
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
        
        // Converts the collateral supply to deposit supply
        pub fn convert_to_deposit(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal)
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_collateral_pool: Option<&CollateralPool> = self.collateral_pools.get(&token_requested);
            match optional_collateral_pool {
                Some (collateral_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        collateral_pool.convert_to_deposit(user_id, token_requested, amount);
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

        pub fn borrow(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal, fees: Bucket) -> Bucket
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        let return_borrow: Bucket = lending_pool.borrow(user_id, token_requested, amount, fees);
                        return_borrow
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[Borrow]: Pool for {:?} doesn't exist.", token_requested);
                    let empty_bucket: Bucket = self.access_vault.take(0);
                    empty_bucket
                }
            }
        }

        pub fn borrow2(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal, fees: Bucket) -> (Bucket, Bucket)
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        let return_borrow: Bucket = lending_pool.borrow(user_id, token_requested, amount, fees);
                        let transient_token = self.flash_loan_auth_vault.authorize(|| {
                            borrow_resource_manager!(self.flash_loan_resource_address).mint_non_fungible(
                                &NonFungibleId::random(),
                                FlashLoan {
                                    amount_due: amount,
                                    borrow_count: 1,
                                },
                            )
                        });
                        
                        (return_borrow, transient_token)
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[Borrow]: Pool for {:?} doesn't exist.", token_requested);
                    let empty_bucket1: Bucket = self.access_vault.take(0);
                    let empty_bucket2: Bucket = self.access_vault.take(0);
                    (empty_bucket1, empty_bucket2)
                }
            }
        }

        pub fn flash_borrow(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Decimal, flash_loan: Proof, fees: Bucket) -> Bucket
        {
            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Assert that flash transient token came from this protocol
            assert_eq!(flash_loan.resource_address(), self.flash_loan_resource_address, "Must send in the correct transient token.");

            // Attempting to get the lending pool component associated with the provided address pair.
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                    let return_borrow: Bucket = lending_pool.borrow(user_id, token_requested, amount, fees);
                    // Updates the flash loan token
                    let borrow_count = 1;
                    self.update_transient_token(&flash_loan, &amount, &borrow_count);
                    return_borrow
                }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[Borrow]: Pool for {:?} doesn't exist.", token_requested);
                    let empty_bucket: Bucket = self.access_vault.take(Decimal::zero());
                    // How to make sure that no changes were made on the flash loan token?
                    empty_bucket
                }
            }
        }

        pub fn create_transient_token(&mut self) -> Bucket {

            let transient_token = self.flash_loan_auth_vault.authorize(|| {
                borrow_resource_manager!(self.flash_loan_resource_address).mint_non_fungible(
                    &NonFungibleId::random(),
                    FlashLoan {
                        amount_due: Decimal::zero(),
                        borrow_count: 0,
                    },
                )
            });
            transient_token
        }

        fn update_transient_token(&mut self, flash_loan: &Proof, borrow_amount: &Decimal, borrow_count: &u8) {
            let mut flash_loan_data: FlashLoan = flash_loan.non_fungible().data();
            flash_loan_data.amount_due += *borrow_amount;
            flash_loan_data.borrow_count += borrow_count;
            self.flash_loan_auth_vault.authorize(|| flash_loan.non_fungible().update_data(flash_loan_data));
        }

        pub fn check_transient_data(&self, flash_loan: Proof) {
            let flash_loan_data: FlashLoan = flash_loan.non_fungible().data();
            let amount_due = flash_loan_data.amount_due;
            let borrow_count = flash_loan_data.borrow_count;
            let balance_statement = info!("The amount borrowed is: {}. The borrow count is {}", amount_due, borrow_count);
        }

        // Works but doesn't check lien and doesnt reduce your balance
        pub fn redeem(&mut self, user_auth: Proof, token_reuqested: ResourceAddress, amount: Decimal, lp_tokens: Bucket) -> Bucket {

            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Assert that transient token has been burnt?

            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_reuqested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_reuqested);       
                        let return_bucket: Bucket = lending_pool.redeem(user_id, token_reuqested, amount, lp_tokens);
                        return_bucket
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_reuqested);
                    let empty_bucket: Bucket = self.access_vault.take(0);
                    empty_bucket
                }
            }
        }

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
                    let empty_bucket: Bucket = self.access_vault.take(0);
                    empty_bucket
                }
            }
        }

        // Think about design of flash repay
        pub fn flash_repay(&mut self, user_auth: Proof, token_requested: ResourceAddress, amount: Bucket, flash_loan: Bucket) -> Bucket {

            // Checks if the user exists
            let user_id = self.get_user(&user_auth);

            // Checks if the token resources are the same
            assert_eq!(token_requested, amount.resource_address(), "Token requested and token deposited must be the same.");

            let flash_loan_data: FlashLoan = flash_loan.non_fungible().data();
            // Can there be a way in which flash loans are partially repaid?
            assert!(amount.amount() >= flash_loan_data.amount_due, "Insufficient repayment given for your loan!");

            // Checks if flash loan bucket is empty

            // Repay fully or partial?
            let optional_lending_pool: Option<&LendingPool> = self.lending_pools.get(&token_requested);
            match optional_lending_pool {
                Some (lending_pool) => { // If it matches it means that the liquidity pool exists.
                    info!("[Lending Protocol Supply Tokens]: Pool for {:?} already exists. Adding supply directly.", token_requested);
                        let return_bucket: Bucket = lending_pool.repay(user_id, token_requested, amount);
                        self.flash_loan_auth_vault.authorize(|| flash_loan.burn());
                        return_bucket
                    }
                None => { // If this matches then there does not exist a liquidity pool for this token pair
                    // In here we are creating a new liquidity pool for this token pair since we failed to find an 
                    // already existing liquidity pool. The return statement below might seem somewhat redundant in 
                    // terms of the two empty buckets being returned, but this is done to allow for the add liquidity
                    // method to be general and allow for the possibility of the liquidity pool not being there.
                    info!("[DEX Add Liquidity]: Pool for {:?} doesn't exist. Creating a new one.", token_requested);
                    let empty_bucket: Bucket = self.access_vault.take(0);
                    empty_bucket
                }
            }
        }
    }
}