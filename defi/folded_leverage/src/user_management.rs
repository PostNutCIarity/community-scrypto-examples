use scrypto::prelude::*;
use crate::lending_pool::*;

// How to prevent people from simply being able to add the balance?

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct User {
    #[scrypto(mutable)]
    deposit_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    borrow_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    collateral_ratio: HashMap<ResourceAddress, Decimal>,
}


blueprint! {
    struct UserManagement {
        // Vault that holds the authorization badge
        user_badge_vault: Vault,
        // Collects User Address
        nft_address: ResourceAddress,
        user_record: HashMap<NonFungibleId, User>,
        allowed_resource: Vec<ResourceAddress>,
        access_vault: Vault,
    }

    impl UserManagement {
        pub fn new(allowed: ResourceAddress, access: Bucket) -> ComponentAddress {

            let access_rules: AccessRules = AccessRules::new().method("register_resource", rule!(require(allowed))).default(rule!(allow_all));

            // Badge that will be stored in the component's vault to update user state.
            let lending_protocol_user_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("user", "Lending Protocol User Badge")
                .initial_supply(1);

            // NFT description for user identification. 
            let nft_address = ResourceBuilder::new_non_fungible()
                .metadata("user", "Lending Protocol User")
                .mintable(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .updateable_non_fungible_data(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .no_initial_supply();
            
            return Self {
                user_badge_vault: Vault::with_bucket(lending_protocol_user_badge),
                nft_address: nft_address,
                user_record: HashMap::new(),
                allowed_resource: Vec::from([allowed]),
                access_vault: Vault::with_bucket(access),
            }
            .instantiate()
            .add_access_check(access_rules)
            .globalize()
        }

        // Creates a new user for the lending protocol.
        // User is created to track the deposit balance, borrow balance, and the token of each.
        // Token is registered by extracting the resource address of the token they deposited.
        // Users are not given a badge. Badge is used by the protocol to update the state. Users are given an NFT to identify as a user.

        // Seems to not be giving me NFT 
        pub fn new_user(&mut self) -> Bucket {

            // Mint NFT to give to users as identification 
            let user_nft = self.user_badge_vault.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.nft_address);
                resource_manager.mint_non_fungible(
                    // The User id
                    &NonFungibleId::random(),
                    // The User data
                    User {
                        borrow_balance: HashMap::new(),
                        deposit_balance: HashMap::new(),
                        collateral_ratio: HashMap::new(),
                    },
                )
            });
            {let user_id: NonFungibleId = user_nft.non_fungible::<User>().id();
                let user: User = user_nft.non_fungible().data();
                self.user_record.insert(user_id, user);}

            // Returns NFT to user
            return user_nft
        }

        pub fn get_nft(&self) -> ResourceAddress {
            return self.nft_address;
        }

        pub fn register_resource(&mut self, resource_address: ResourceAddress) {
            self.allowed_resource.push(resource_address)
        }
        // Not even sure if this is something should implement
        // What if NFT state updates? Does it update along with the hashmap?
        fn find_user(&self, user_id: &NonFungibleId) -> bool {
            return self.user_record.contains_key(&user_id)
        }

        fn assert_user_exist(&self, user_id: &NonFungibleId) {
            assert!(self.find_user(user_id), "User does not exist.");
        }        

        // Need help on error regarding the unwrap 06/01/22
        // Need to think about this more whether it needs to equal exactly zero
        fn check_lien(&self, user_id: &NonFungibleId, token_requested: &ResourceAddress) {
            // Check if deposit withdrawal request has no lien
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert_eq!(nft_data.borrow_balance.get(&token_requested).unwrap_or(&Decimal::zero()), &Decimal::zero(), "User need to repay loan")
        }

        // Adds the deposit balance
        // Checks if the user already a record of the resource or not
        // Requires a NonFungibleId so the method knows which NFT to update the data
        // The lending pool deposit method mints a transient resource that contains the amount that has been deposited to the pool
        // The transient resource address is then registered to this component where add_deposit_balance checks whether the transient resource token that has been passed
        // Is the same as the transient resource that was created in the lending pool component
        // The NFT data is then updated and the transient resource is burnt.
        pub fn add_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(amount, transient_token.amount(), "Incorrect amount.");

            // Register lending pool


            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            if nft_data.deposit_balance.contains_key(&address) {
                *nft_data.deposit_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                nft_data.deposit_balance.insert(address, amount);
            };

            // Added to check whether the transient token is being burnt
            self.access_vault.authorize(|| transient_token.burn());
            
            
            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        // Check and understand the logic here - 06/01/2022
        // Does not decrease balance 06/02/22
        pub fn decrease_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, redeem_amount: Decimal, transient_token: Bucket) -> Decimal {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Asserts user exists
            self.assert_user_exist(&user_id);
            
            // Check lien - 06/01/22 - Make sure this makes sense
            self.check_lien(&user_id, &address);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_deposit_resource_exists(&user_id, &address);

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(redeem_amount, transient_token.amount(), "Incorrect amount.");

            self.access_vault.authorize(|| transient_token.burn());

            // Retrieves resource manager to find user 
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            // If the repay amount is larger than the borrow balance, returns the excess to the user. Otherwise, balance simply reduces.
            let mut borrow_balance = *nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero());

            if borrow_balance < redeem_amount {
                let to_return = redeem_amount - borrow_balance;
                // Will value be negative?
                // Update 06/2/22 - tryna isolate why it's not updating redeem balance
                let mut update_nft_data: User = resource_manager.get_non_fungible_data(&user_id);
                *update_nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) -= redeem_amount;
                self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, update_nft_data));
                return to_return
            }
            else {
                borrow_balance -= redeem_amount;
                self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
                return Decimal::zero()
            };
        }
        
        // Checks the user's total tokens and deposit balance of those tokens


        // Grabs the deposit balance of a specific token
        fn check_token_deposit_balance(&self, user_id: &NonFungibleId, token_address: ResourceAddress) -> Decimal {
            // Retrieves NonFungible ID
            let resource_manager = borrow_resource_manager!(self.nft_address);
            // Gets value of User
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            return *nft_data.deposit_balance.get(&token_address).unwrap_or(&Decimal::zero());
        }

        pub fn deposit_resource_exists(&self, user_auth: Proof, address: ResourceAddress) -> bool {
            let user_badge_data: User = user_auth.non_fungible().data();
            return user_badge_data.deposit_balance.contains_key(&address);
        }

        fn assert_deposit_resource_exists(&self, user_id: &NonFungibleId, address: &ResourceAddress) {
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert!(nft_data.deposit_balance.contains_key(&address), "This token resource does not exist in your deposit balance.")
        }

        // Adds the borrow balance
        // Checks if the user already a record of the resource or not
        pub fn add_borrow_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(amount, transient_token.amount(), "Incorrect amount.");

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            if nft_data.borrow_balance.contains_key(&address) {
                *nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) += amount;
                *nft_data.collateral_ratio.get_mut(&address).unwrap_or(&mut Decimal::zero()) += self.get_current_collateral_ratio(&user_id, address, amount).unwrap_or(Decimal::zero());
            }
            else {
                nft_data.borrow_balance.insert(address, amount);
                *nft_data.collateral_ratio.get_mut(&address).unwrap_or(&mut Decimal::zero()) += self.get_current_collateral_ratio(&user_id, address, amount).unwrap_or(Decimal::zero());
            };
            
            self.access_vault.authorize(|| transient_token.burn());

            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        pub fn decrease_borrow_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, repay_amount: Decimal, transient_token: Bucket) -> Decimal {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_borrow_resource_exists(&user_id, &address);

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(repay_amount, transient_token.amount(), "Incorrect amount.");
            self.access_vault.authorize(|| transient_token.burn());

            // Retrieves resource manager to find user 
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            // If the repay amount is larger than the borrow balance, returns the excess to the user. Otherwise, balance simply reduces.
            let borrow_balance = *nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero());

            if borrow_balance < repay_amount {
                let to_return = repay_amount - borrow_balance;
                let mut update_nft_data: User = resource_manager.get_non_fungible_data(&user_id);
                *update_nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) = Decimal::zero();
                self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, update_nft_data));
                return to_return
            }
            else {
                *nft_data.borrow_balance.get_mut(&address).unwrap() -= repay_amount;
                self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
                return Decimal::zero()
            };
        }

        pub fn check_borrow_balance(&self, user_auth: Proof) { // This way or check_deposit_balance?
            let user_badge_data: User = user_auth.non_fungible().data();
            for (token, amount) in &user_badge_data.borrow_balance {
                println!("{}: \"{}\"", token, amount)
            }
        }

        fn check_token_borrow_balance(&self, user_id: &NonFungibleId, token_address: ResourceAddress) -> Decimal {
            // Retrieves NonFungible ID
            let resource_manager = borrow_resource_manager!(self.nft_address);
            // Gets value of User
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            return *nft_data.borrow_balance.get(&token_address).unwrap_or(&Decimal::zero());
        }

        fn assert_borrow_resource_exists(&self, user_id: &NonFungibleId, address: &ResourceAddress) {
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert!(nft_data.borrow_balance.contains_key(&address), "This token resource does not exist in your borrow balance.")
        }

        pub fn get_collateral_ratio(&self, user_id: NonFungibleId, token_address: ResourceAddress) -> Decimal {

            let collateral_ratio: Decimal = self.check_token_borrow_balance(&user_id, token_address) / self.check_token_deposit_balance(&user_id, token_address);
            return collateral_ratio
        }

        fn get_current_collateral_ratio(&self, user_id: &NonFungibleId, token_address: ResourceAddress, amount: Decimal) -> Option<Decimal> {
            if self.check_token_borrow_balance(&user_id, token_address).is_zero() {
                None
            } else {

            let collateral_ratio: Decimal = (self.check_token_borrow_balance(&user_id, token_address) + amount) / self.check_token_deposit_balance(&user_id, token_address);
                Some(collateral_ratio)
            }
        }
    }
}
