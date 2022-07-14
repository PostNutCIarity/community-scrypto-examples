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
        sbt_badge_vault: Vault,
        /// Collects User Address
        sbt_address: ResourceAddress,
        /// This is the user record registry. It is meant to allow people to query the users that belongs to this protocol.
        user_record: HashMap<NonFungibleId, User>,
        /// Keeps a record of wallet addresses to ensure that maps 1 SBT to 1 Wallet.
        account_record: Vec<ComponentAddress>,
    }

    /// Instantiates the User Management component. This is instantiated through the main router component. 
    /// At instantiation, the component requires the resource address of the authorization badge that is minted
    /// by the main router component. This logic simply says that only components with the access token is allowed
    /// to call the "register_resource" method. This method is used by the pools to register the transient tokens
    /// that are minted as a result of interacting with the pool (i.e to borrow). This is required to ensure there 
    /// are proper access controls to updating the User NFT. 
    impl UserManagement {
        pub fn new(
            access_badge_address: ResourceAddress
        ) -> ComponentAddress
        {
            let access_rules: AccessRules = AccessRules::new()
            .method("new_user", rule!(require(access_badge_address)))
            .method("inc_credit_score", rule!(require(access_badge_address)))
            .method("dec_credit_score", rule!(require(access_badge_address)))
            .method("add_deposit_balance", rule!(require(access_badge_address)))
            .method("decrease_deposit_balance", rule!(require(access_badge_address)))
            .method("increase_borrow_balance", rule!(require(access_badge_address)))
            .method("decrease_borrow_balance", rule!(require(access_badge_address)))
            .method("add_collateral_balance", rule!(require(access_badge_address)))
            .method("decrease_collateral_balance", rule!(require(access_badge_address)))
            .method("inc_paid_off", rule!(require(access_badge_address)))
            .method("inc_default", rule!(require(access_badge_address)))
            .method("insert_loan", rule!(require(access_badge_address)))
            .method("close_loan", rule!(require(access_badge_address)))
            .method("convert_deposit_to_collateral", rule!(require(access_badge_address)))
            .method("convert_collateral_to_deposit", rule!(require(access_badge_address)))
            .default(rule!(allow_all));

            // Badge that will be stored in the component's vault to provide authorization to update the User NFT.
            let sbt_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("user", "Lending Protocol User Badge")
                .initial_supply(1);

            // NFT description for user identification. 
            let sbt_data = ResourceBuilder::new_non_fungible()
                .metadata("user", "Lending Protocol User")
                .mintable(rule!(require(sbt_badge.resource_address())), LOCKED)
                .burnable(rule!(require(sbt_badge.resource_address())), LOCKED)
                .restrict_withdraw(rule!(deny_all), LOCKED)
                .updateable_non_fungible_data(rule!(require(sbt_badge.resource_address())), LOCKED)
                .no_initial_supply();
            
            return Self {
                sbt_badge_vault: Vault::with_bucket(sbt_badge),
                sbt_address: sbt_data,
                user_record: HashMap::new(),
                account_record: Vec::new(),
            }
            .instantiate()
            .add_access_check(access_rules)
            .globalize()
        }

        /// Creates a new user for the lending protocol.
        /// 
        /// This method is used to create a new user for DegenFi. A "Soul Bound Token" (SBT) is
        /// created and sent to the user's wallet which cannot be transferred or burnt. The SBT tracks
        /// user interactions within the protocol. Its major use case is to attempt to create a borrowing
        /// track record to underwrite the user's credit worthines. The user has to submit their
        /// wallet's component address to prevent the creation of multiple SBTs. Most of the protocol's
        /// method will require users to submit a proof of their SBT in order to use the protocol. 
        /// 
        /// This method performs a few checks before a new user is created, these are:
        /// 
        /// * **Check 1:** Checks whether the wallet address submitted has already been submitted or not. 
        /// 
        /// # Arguments: 
        /// 
        /// * `account_address` (ComponentAddress) - The user's wallet address to ensure the user cannot create multiple
        /// SBTs.
        /// 
        /// # Returns:
        /// 
        /// * `Bucket` - This is the SBT the user receives from creating a new user.
        pub fn new_user(&mut self, account_address: ComponentAddress) -> Bucket {

            // Checks whether the account address has already registered an SBT
            assert_ne!(self.account_record.contains(&account_address), true, "SBT already created for this account.");
            
            // Mint NFT to give to users as identification 
            let user_nft = self.sbt_badge_vault.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.sbt_address);
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

        fn call_resource_mananger(
            &self,
            user_id: &NonFungibleId
        ) -> User
        {
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let sbt: User = resource_manager.get_non_fungible_data(&user_id);
            return sbt
        }

        fn authorize_update(
            &mut self,
            user_id: &NonFungibleId,
            sbt_data: User
        )
        {
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            self.sbt_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, sbt_data));
        }

        /// Gets the SBT resource address.
        /// 
        /// This method is used to retrieve the resource address of the SBT. It is used for other
        /// Blueprints to view the SBT's data.
        /// 
        /// This method does not perform any checks.
        /// 
        /// # Arguments: 
        /// 
        /// This method does not require any methods to be passed through.
        /// 
        /// # Returns:
        /// 
        /// This method does not return any assets.
        pub fn get_sbt(
            &self
        ) -> ResourceAddress 
        {
            return self.sbt_address;
        }
        
        /// Takes in the NonFungibleId and reveals whether this NonFungibleId belongs to the protocol.
        fn find_user(
            &self,
            user_id: &NonFungibleId
        ) -> bool 
        {
            return self.user_record.contains_key(&user_id)
        }

        /// Asserts that the user must belong to the is protocol.
        fn assert_user_exist(
            &self,
            user_id: &NonFungibleId
        ) 
        {
            assert!(self.find_user(user_id), "User does not exist.");
        }        

        fn check_lien(&self,
            user_id: &NonFungibleId,
            token_requested: &ResourceAddress
        ) 
        {
            // Check if deposit withdrawal request has no lien
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert_eq!(sbt_data.borrow_balance.get(&token_requested).unwrap_or(&Decimal::zero()), &Decimal::zero(), "User need to repay loan")
        }

        pub fn inc_credit_score(
            &mut self,
            user_id: NonFungibleId,
            amount: u64
        )
        {
            // Calls the resource manager.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            // Increases the credit score.
            sbt_data.credit_score += amount;

            // Authorizes the update.
            self.authorize_update(&user_id, sbt_data);
        }

        pub fn dec_credit_score(
            &mut self,
            user_id: NonFungibleId,
            amount: u64
        )
        {
            // Calls the resource manager.
            let mut sbt_data = self.call_resource_mananger(&user_id);

            // Increases the credit score.
            sbt_data.credit_score -= amount;

            // Authorizes the update.
            self.authorize_update(&user_id, sbt_data);
        }

        /// Adds the deposit balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn add_deposit_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            amount: Decimal
        ) 
        {
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

        /// Decreases the deposit balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn decrease_deposit_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            redeem_amount: Decimal
        ) 
        {
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
        
        pub fn deposit_resource_exists(
            &self,
            user_auth: Proof,
            address: ResourceAddress
        ) -> bool
        {
            let user_badge_data: User = user_auth.non_fungible().data();
            return user_badge_data.deposit_balance.contains_key(&address);
        }

        fn assert_deposit_resource_exists(
            &self,
            user_id: &NonFungibleId,
            address: &ResourceAddress
        ) 
        {
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert!(sbt_data.deposit_balance.contains_key(&address), "This token resource does not exist in your deposit balance.")
        }

        /// Adds the borrow balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn increase_borrow_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            amount: Decimal
        ) 
        {
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


        /// Decreases the borrow balance of the User SBT.
        /// 
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
        pub fn decrease_borrow_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            repay_amount: Decimal
        ) -> Decimal
        {
            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_borrow_resource_exists(&user_id, &address);

            // Retrieves resource manager to find user 
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let mut sbt_data: User = resource_manager.get_non_fungible_data(&user_id);

            // If the repay amount is larger than the borrow balance, returns the excess to the user. Otherwise, balance simply reduces.
            let borrow_balance = *sbt_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero());

            if borrow_balance < repay_amount {
                let to_return = repay_amount - borrow_balance;
                let mut update_sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
                *update_sbt_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) = Decimal::zero();
                self.sbt_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, update_sbt_data));
                return to_return
            }
            else {
                *sbt_data.borrow_balance.get_mut(&address).unwrap() -= repay_amount;
                self.sbt_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, sbt_data));
                return Decimal::zero()
            };
        }

        /// Increases the counter of paid off loans of the SBT User.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the paid off counter increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn inc_paid_off(
            &mut self,
            user_id: NonFungibleId
        )
        {
            // Asserts user exists
            self.assert_user_exist(&user_id);

            // Retrieves resource manager to find user 
            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.paid_off += 1;

            self.authorize_update(&user_id, sbt_data);
        }

        /// Increases the default counter of the SBT User.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn inc_default(
            &mut self,
            user_id: NonFungibleId
        ) 
        {
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
        pub fn insert_loan(
            &mut self,
            user_id: NonFungibleId,
            token_address: ResourceAddress,
            loan_id: NonFungibleId
        ) 
        {
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

        pub fn close_loan(
            &mut self,
            user_id: NonFungibleId,
            token_address: ResourceAddress,
            loan_id: NonFungibleId
        ) 
        {
            // Asserts user exists
            self.assert_user_exist(&user_id);

            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.open_loans.remove_entry(&token_address);
            sbt_data.closed_loans.insert(token_address, loan_id);

            self.authorize_update(&user_id, sbt_data);
        }
        

        pub fn check_borrow_balance(
            &self, 
            user_auth: Proof
        )
        {
            let user_badge_data: User = user_auth.non_fungible().data();
            for (token, amount) in &user_badge_data.borrow_balance {
                println!("{}: \"{}\"", token, amount)
            }
        }

        fn assert_borrow_resource_exists(
            &self, 
            user_id: &NonFungibleId, 
            address: &ResourceAddress
        )
        {
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert!(sbt_data.borrow_balance.contains_key(&address), "This token resource does not exist in your borrow balance.")
        }

        /// Converts the deposit balance to the collateral balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn convert_deposit_to_collateral(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            amount: Decimal
        )
        {
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

        pub fn convert_collateral_to_deposit(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            amount: Decimal
        ) 
        {
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

        /// Adds the borrow balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn add_collateral_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            amount: Decimal)
            {
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

        /// Adds the borrow balance of the User SBT.
        /// 
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        ///
        /// * **Check 1:** Checks that there is a user that exist for the NonFungibleId passed.
        /// 
        /// * **Check 2:** Checks if the user has borrowed from the resource address before, if not, inserts the resource address
        /// and amount to the HashMap.
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
        /// # Returns:
        /// 
        /// The method does not return any assets.
        pub fn decrease_collateral_balance(
            &mut self,
            user_id: NonFungibleId,
            address: ResourceAddress,
            redeem_amount: Decimal
        ) -> Decimal
        {
            // Asserts user exists
            self.assert_user_exist(&user_id);
            
            // Check lien - 06/01/22 - Make sure this makes sense
            //self.check_lien(&user_id, &address);

            // Asserts that the user must have an existing borrow balance of the resource.
            self.assert_deposit_resource_exists(&user_id, &address);

            // Retrieves resource manager to find user 
            let resource_manager = borrow_resource_manager!(self.sbt_address);
            let mut sbt_data: User = resource_manager.get_non_fungible_data(&user_id);

            // If the repay amount is larger than the borrow balance, returns the excess to the user. Otherwise, balance simply reduces.
            let mut borrow_balance = *sbt_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero());

            if borrow_balance < redeem_amount {
                let to_return = redeem_amount - borrow_balance;
                let mut update_sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
                *update_sbt_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) -= redeem_amount;
                self.sbt_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, update_sbt_data));
                return to_return
            }
            else {
                borrow_balance -= redeem_amount;
                self.sbt_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, sbt_data));
                return Decimal::zero()
            };
        }

        pub fn interest_modifier(
            &self,
            user_id: NonFungibleId
        ) -> Decimal 
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

        pub fn collaterization_modifier(
            &self,
            user_id: NonFungibleId
        ) -> Decimal 
        {
            let sbt_data = self.call_resource_mananger(&user_id);
            let credit_score = sbt_data.credit_score;
            if credit_score >= 100 && credit_score < 200 {
                return dec!(".05")
            } else if credit_score >= 200 && credit_score < 300 {
                return dec!(".10")
            } else if credit_score >= 300 {
                return dec!(".15")
            } else {
                return dec!("0.0")
            }
        }

        /// Allows user to add to their credit score.
        ///
        /// This method is used to allow users add to their credit score for demonstration purpose.
        /// 
        /// This method does not perform any checks.
        /// 
        /// # Arguments:
        /// 
        /// * `user_auth` (Proof) - A proof that proves that the depositer is a user that belongs to this protocol.
        /// * `credit_score` (u64) - The credit score amount user wants to add.
        /// 
        /// # Returns:
        /// 
        /// This method does not return any assets.
        pub fn set_credit_score(
            &mut self,
            user_id: NonFungibleId,
            credit_score: u64
        )
        {
            let mut sbt_data = self.call_resource_mananger(&user_id);
            sbt_data.credit_score = credit_score;
            self.authorize_update(&user_id, sbt_data);
        }

        /// Allows user to pull their SBT data.
        ///
        /// This method is used to allow users retrieve their SBT data. I suppose users cannot retrieve SBT data
        /// of other users yet.
        /// 
        /// This method does not perform any checks.
        /// 
        /// # Arguments:
        /// 
        /// * `user_auth` (Proof) - A proof that proves that the depositer is a user that belongs to this protocol.
        /// 
        /// # Returns:
        /// 
        /// This method does not return any assets.
        pub fn get_sbt_info(
            &self,
            user_id: NonFungibleId
        )
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
