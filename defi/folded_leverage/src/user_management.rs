use scrypto::prelude::*;
use crate::structs::{User};

blueprint! {
/// Users who know their NFT ID has access to this component to view their data.
/// Users can not change their data. Only interacting through the pools can.
/// Changing deposit, borrow, and colalteral balance data can only be triggered through interacting with the pool.
/// The Pool mints a transient token that will be sent to the User Management component.
/// The User Management component has a protected method through which the pool can call to register the resource address
/// Of the transient token. The transient token that is passed to the method call in the User Management component
/// is then checked to ensure that the transient token was indeed minted from the pool.
    struct UserManagement {
        /// Vault that holds the authorization badge
        user_badge_vault: Vault,
        /// Collects User Address
        nft_address: ResourceAddress,
        /// This is the user record registry. It is meant to allow people to query the users that belongs to this protocol.
        pub user_record: HashMap<NonFungibleId, User>,
        account_record: Vec<ComponentAddress>,
    }

    /// Instantiates the User Management component. This is instantiated through the main router component. 
    /// At instantiation, the component requires the resource address of the authorization badge that is minted
    /// by the main router component. This logic simply says that only components with the access token is allowed
    /// to call the "register_resource" method. This method is used by the pools to register the transient tokens
    /// that are minted as a result of interacting with the pool (i.e to borrow). This is required to ensure there 
    /// are proper access controls to updating the User NFT. 
    impl UserManagement {
        pub fn new(allowed: ResourceAddress) -> ComponentAddress {

            let access_rules: AccessRules = AccessRules::new()
            .method("new_user", rule!(require(allowed)))
            .method("inc_credit_score", rule!(require(allowed)))
            .method("dec_credit_score", rule!(require(allowed)))
            .method("add_deposit_balance", rule!(require(allowed)))
            .method("decrease_deposit_balance", rule!(require(allowed)))
            .method("increase_borrow_balance", rule!(require(allowed)))
            .method("decrease_borrow_balance", rule!(require(allowed)))
            .method("add_collateral_balance", rule!(require(allowed)))
            .method("decrease_collateral_balance", rule!(require(allowed)))
            .method("inc_paid_off", rule!(require(allowed)))
            .method("inc_default", rule!(require(allowed)))
            .method("insert_loan", rule!(require(allowed)))
            .method("close_loan", rule!(require(allowed)))
            .method("convert_deposit_to_collateral", rule!(require(allowed)))
            .method("convert_collateral_to_deposit", rule!(require(allowed)))
            .default(rule!(allow_all));

            // Badge that will be stored in the component's vault to provide authorization to update the User NFT.
            let lending_protocol_user_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("user", "Lending Protocol User Badge")
                .initial_supply(1);

            // NFT description for user identification. 
            let nft_address = ResourceBuilder::new_non_fungible()
                .metadata("user", "Lending Protocol User")
                .mintable(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .restrict_withdraw(rule!(deny_all), LOCKED)
                .updateable_non_fungible_data(rule!(require(lending_protocol_user_badge.resource_address())), LOCKED)
                .no_initial_supply();
            
            return Self {
                user_badge_vault: Vault::with_bucket(lending_protocol_user_badge),
                nft_address: nft_address,
                user_record: HashMap::new(),
                account_record: Vec::new(),
            }
            .instantiate()
            .add_access_check(access_rules)
            .globalize()
        }

        // Creates a new user for the lending protocol.
        // User is created to track the deposit balance, borrow balance, and the token of each.
        // Token is registered by extracting the resource address of the token they deposited.
        // Users are not given a badge. Badge is used by the protocol to update the state. Users are given an NFT to identify as a user.
        pub fn new_user(&mut self, account_address: ComponentAddress) -> Bucket {

            // Checks whether the account address has already registered an SBT
            assert_ne!(self.account_record.contains(&account_address), true, "SBT already created for this account.");
            
            // Mint NFT to give to users as identification 
            let user_nft = self.user_badge_vault.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.nft_address);
                resource_manager.mint_non_fungible(
                    // The User id
                    &NonFungibleId::random(),
                    // The User data
                    User {
                        credit_score: 0,
                        borrow_balance: HashMap::new(),
                        deposit_balance: HashMap::new(),
                        collateral_balance: HashMap::new(),
                        open_loans: HashMap::new(),
                        closed_loans: HashMap::new(),
                        defaults: 0,
                        paid_off: 0,
                    },
                )
            });
            
            // Registers the user to the user_record HashMap
            {
                let user_id: NonFungibleId = user_nft.non_fungible::<User>().id();
                let user: User = user_nft.non_fungible().data();
                self.user_record.insert(user_id, user);
                self.account_record.push(account_address);
            }

            // Returns NFT to user
            return user_nft
        }

        fn call_resource_mananger(&self, user_id: &NonFungibleId) -> User {
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let sbt: User = resource_manager.get_non_fungible_data(&user_id);
            return sbt
        }

        fn authorize_update(&mut self, user_id: &NonFungibleId, sbt_data: User) {
            let resource_manager = borrow_resource_manager!(self.nft_address);
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, sbt_data));
        }

        /// Currently simply a way for other components to get the resource address of the NFT
        /// so that it can take informationa bout the NFT itself.
        pub fn get_nft(&self) -> ResourceAddress {
            return self.nft_address;
        }
        
        /// Takes in the NonFungibleId and reveals whether this NonFungibleId belongs to the protocol.
        fn find_user(&self, user_id: &NonFungibleId) -> bool {
            return self.user_record.contains_key(&user_id)
        }

        /// Asserts that the user must belong to the is protocol.
        fn assert_user_exist(&self, user_id: &NonFungibleId) {
            assert!(self.find_user(user_id), "User does not exist.");
        }        

        /// Need help on error regarding the unwrap 06/01/22
        /// Need to think about this more whether it needs to equal exactly zero
        fn check_lien(&self, user_id: &NonFungibleId, token_requested: &ResourceAddress) 
        {
            // Check if deposit withdrawal request has no lien
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert_eq!(nft_data.borrow_balance.get(&token_requested).unwrap_or(&Decimal::zero()), &Decimal::zero(), "User need to repay loan")
        }

        pub fn inc_credit_score(&mut self, user_id: NonFungibleId, amount: u64)
        {
            // Calls the resource manager.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            // Increases the credit score.
            sbt_data.credit_score += amount;

            // Authorizes the update.
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn dec_credit_score(&mut self, user_id: NonFungibleId, amount: u64)
        {
            // Calls the resource manager.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            // Increases the credit score.
            sbt_data.credit_score -= amount;

            // Authorizes the update.
            self.authorize_update(&user_id, sbt_data);
        }

        /// Adds the deposit balance
        /// Checks if the user already a record of the resource or not
        /// Requires a NonFungibleId so the method knows which NFT to update the data
        /// The lending pool deposit method mints a transient resource that contains the amount that has been deposited to the pool
        /// The transient resource address is then registered to this component where add_deposit_balance checks whether the transient resource token that has been passed
        /// Is the same as the transient resource that was created in the lending pool component
        /// The NFT data is then updated and the transient resource is burnt.
        pub fn add_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            if sbt_data.deposit_balance.contains_key(&address) {
                *sbt_data.deposit_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                sbt_data.deposit_balance.insert(address, amount);
            };
            
            // Commits state
            self.authorize_update(&user_id, sbt_data);
        }

        /// Check and understand the logic here - 06/01/2022
        pub fn decrease_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, redeem_amount: Decimal) {

            // Asserts user exists
            self.assert_user_exist(&user_id);
            
            // Check lien - 06/01/22 - Make sure this makes sense
            self.check_lien(&user_id, &address);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_deposit_resource_exists(&user_id, &address);

            // Retrieves resource manager to find user 
            let mut sbt_data = self.call_resource_mananger(&user_id);
            assert!(sbt_data.deposit_balance.contains_key(&address), "Must have this deposit resource to withdraw");
            *sbt_data.deposit_balance.get_mut(&address).unwrap() -= redeem_amount;

            assert!(sbt_data.deposit_balance.get_mut(&address).unwrap() >= &mut Decimal::zero(), "Deposit balance cannot be negative.");

            self.authorize_update(&user_id, sbt_data);
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

        /// Adds the borrow balance of the User NFT.
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        /// 
        /// * **Check 1:** Checks to ensure that the transient tokens passed do indeed belong to the pools of the protocol.
        /// * **Check 2:** Checks to ensure that the amount in the transient token is the same amount that is required to update the
        /// borrow balance.
        /// * **Check 3:** Checks to ensure that the user belongs to this protocol.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `address` (ResourceAddress) - This is the token address of the borrow balance that needs to be updated.
        /// 
        /// * `amount` (Decimal) - This is the amount of the borrow balance that needs to be updated.
        /// 
        /// * `transient_token` (Bucket) - The transient token that is passed to this method is used to ensure that no one can
        /// simply change the data of the NFT. The NFT data can only be changed by interacting with the pool. The pool methods will
        /// mint a transient token to be sent to this method to ensure that the user has interacted with the pool to cause for the
        /// NFT data to be updated.
        /// 
        /// # Returns:
        /// 
        /// * `None` - The method simply updates the User NFT
        pub fn increase_borrow_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let mut sbt_data = self.call_resource_mananger(&user_id);
            
            if sbt_data.borrow_balance.contains_key(&address) {
                *sbt_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) += amount;
            } else {
                sbt_data.borrow_balance.insert(address, amount);
            };

            // Commits state
            self.authorize_update(&user_id, sbt_data);
        }


        /// Decreases the borrow balance of the User NFT.
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        /// 
        /// * **Check 1:** Checks to ensure that the transient tokens passed do indeed belong to the pools of the protocol.
        /// * **Check 2:** Checks to ensure that the amount in the transient token is the same amount that is required to update the
        /// borrow balance.
        /// * **Check 3:** Checks to ensure that the user belongs to this protocol.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `address` (ResourceAddress) - This is the token address of the borrow balance that needs to be updated.
        /// 
        /// * `repay_amount` (Decimal) - This is the amount of the borrow balance that needs to be updated to decrease.
        /// 
        /// * `transient_token` (Bucket) - The transient token that is passed to this method is used to ensure that no one can
        /// simply change the data of the NFT. The NFT data can only be changed by interacting with the pool. The pool methods will
        /// mint a transient token to be sent to this method to ensure that the user has interacted with the pool to cause for the
        /// NFT data to be updated.
        /// 
        /// # Returns:
        /// 
        /// * `Decimal` - The number of which is sent to the pool to show how much is owed to the borrower if the borrower overpaid to close out the loan.
        pub fn decrease_borrow_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, repay_amount: Decimal, ) -> Decimal {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_borrow_resource_exists(&user_id, &address);

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

        pub fn inc_paid_off(&mut self, user_id: NonFungibleId) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Retrieves resource manager to find user 
            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.paid_off += 1;

            self.authorize_update(&user_id, sbt_data);
        }

        pub fn inc_default(&mut self, user_id: NonFungibleId) {
            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Retrieves resource manager to find user 
            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.defaults += 1;

            self.authorize_update(&user_id, sbt_data);
        }

        /// This inserts the NonFungibleId of the loan to the Usert NFT.
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a few checks before the User NFT is updated.
        /// 
        /// * **Check 1:** Checks to ensure that the user belongs to this protocol.
        /// * **Check 2: ** Checks to ensure that the loan hasn't already been inputed to the User NFT.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `loan_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the loan.
        /// 
        /// * `transient_token` (Bucket) - The transient token that is passed to this method is used to ensure that no one can
        /// simply change the data of the NFT. The NFT data can only be changed by interacting with the pool. The pool methods will
        /// mint a transient token to be sent to this method to ensure that the user has interacted with the pool to cause for the
        /// NFT data to be updated.
        /// 
        /// # Returns:
        /// 
        /// * `None` - This method simply updates the User NFT.
        pub fn insert_loan(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, loan_id: NonFungibleId) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            let mut sbt_data = self.call_resource_mananger(&user_id);

            if sbt_data.open_loans.contains_key(&token_address) {
                info!("Loan has already been recorded.")
            } else {
                sbt_data.open_loans.insert(token_address, loan_id);
            }

            self.authorize_update(&user_id, sbt_data);
        }

        pub fn close_loan(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, loan_id: NonFungibleId) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.open_loans.remove_entry(&token_address);
            sbt_data.closed_loans.insert(token_address, loan_id);

            self.authorize_update(&user_id, sbt_data);
        }
        

        pub fn check_borrow_balance(&self, user_auth: Proof) { // This way or check_deposit_balance?
            let user_badge_data: User = user_auth.non_fungible().data();
            for (token, amount) in &user_badge_data.borrow_balance {
                println!("{}: \"{}\"", token, amount)
            }
        }

        fn assert_borrow_resource_exists(&self, user_id: &NonFungibleId, address: &ResourceAddress) {
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert!(nft_data.borrow_balance.contains_key(&address), "This token resource does not exist in your borrow balance.")
        }

        pub fn convert_deposit_to_collateral(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal) {

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            // Converts the deposit to collateral balance by subtracting from deposit and adding to collateral balance.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            if sbt_data.collateral_balance.contains_key(&address) {
                *sbt_data.deposit_balance.get_mut(&address).unwrap() -= amount;
                *sbt_data.collateral_balance.get_mut(&address).unwrap() += amount;
            } else {
                *sbt_data.deposit_balance.get_mut(&address).unwrap() -= amount;
                sbt_data.collateral_balance.insert(address, amount);
            };
             
            // Commits state
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn convert_collateral_to_deposit(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal) {

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            // Converts the deposit to collateral balance by subtracting from deposit and adding to collateral balance.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            if sbt_data.deposit_balance.contains_key(&address) {
                *sbt_data.collateral_balance.get_mut(&address).unwrap() -= amount;
                *sbt_data.deposit_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                *sbt_data.collateral_balance.get_mut(&address).unwrap() -= amount;
                sbt_data.deposit_balance.insert(address, amount);
            };
            
            // Commits state
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn add_collateral_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal) {

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            if sbt_data.collateral_balance.contains_key(&address) {
                *sbt_data.collateral_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                sbt_data.collateral_balance.insert(address, amount);
            };
                     
            // Commits state
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn decrease_collateral_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, redeem_amount: Decimal) -> Decimal {

            // Asserts user exists
            self.assert_user_exist(&user_id);
            
            // Check lien - 06/01/22 - Make sure this makes sense
            self.check_lien(&user_id, &address);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_deposit_resource_exists(&user_id, &address);

            // Retrieves resource manager to find user 
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            // If the repay amount is larger than the borrow balance, returns the excess to the user. Otherwise, balance simply reduces.
            let mut borrow_balance = *nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero());

            if borrow_balance < redeem_amount {
                let to_return = redeem_amount - borrow_balance;
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

        pub fn credit_score_modifier(&self, user_id: NonFungibleId) -> Decimal 
        {
            let sbt_data = self.call_resource_mananger(&user_id);
            let credit_score = sbt_data.credit_score;
            if credit_score >= 100 && credit_score < 200 {
                return dec!(".01")
            } else if credit_score >= 200 && credit_score < 300 {
                return dec!(".02")
            } else if credit_score >= 300 {
                return dec!(".03")
            } else {
                return dec!("0.0")
            }
        }

        pub fn credit_score_test(&mut self, user_id: NonFungibleId, credit_score: u64)
        {
            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.credit_score += credit_score;
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn get_sbt_info(&self, user_id: NonFungibleId)
        {
            let sbt_data = self.call_resource_mananger(&user_id);
            let credit_score = sbt_data.credit_score;
            let deposit_balance = sbt_data.deposit_balance;
            let collateral_balance = sbt_data.collateral_balance;
            let borrow_balance = sbt_data.borrow_balance;
            let open_loans = sbt_data.open_loans;
            let closed_loans = sbt_data.closed_loans;
            let defaults = sbt_data.defaults;
            let paid_off = sbt_data.paid_off;

            info!("[User SBT]: Credit Score: {:?}", credit_score);
            info!("[User SBT]: Deposit Balance: {:?}", deposit_balance);
            info!("[User SBT]: Collateral Balance: {:?}", collateral_balance);
            info!("[User SBT]: Borrow Balance: {:?}", borrow_balance);
            info!("[User SBT]: Open Loans: {:?}", open_loans);
            info!("[User SBT]: Closed Loans: {:?}", closed_loans);
            info!("[User SBT]: Number of times liquidated: {:?}", defaults);
            info!("[User SBT]: Number of loans paid off: {:?}", paid_off);
        }
    }
}
