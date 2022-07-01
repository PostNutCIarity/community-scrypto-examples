use scrypto::prelude::*;
use crate::lending_pool::*;

/// User NFT is an NFT that represents users for this protocol. This NFT contains all the records of the user
/// interacting with this protocol. It can be seen as a credit report for the user. It is also used for authorization
/// that this user belongs to the protocol and access protocol features. Users themselves do not have permission to
/// change the data contained within the NFT. It is a non-transferable token or otherwise known as a "Soul Bound Token"
/// or "SBT" for short. The reason to contain deposit, collateral, and borrow balance as a HashMap is for better flexibility
/// and user experience. Especially when it comes to repaying loans. When a loan is paid off, users do not have to worry about
/// sending the wrong NFT, the protocol will simply look at the SBT token and find the loan that the user wants to pay off.
#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct User {
    #[scrypto(mutable)]
    deposit_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    // Should we have collateral_balance: HashMap<(ResourceAddress, NonFungibleID), Decimal> instead?
    collateral_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    borrow_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    loans: BTreeSet<NonFungibleId>,
    defaults: u64,
    paid_off: u64,
}

/// This is an NFT that represents the loan terms. We can consider this NFT as loan documents and hopefully in the future can
/// be represented as legal documents or a digital representation of a legal document. This NFT is given to the borrower.
/// For now its purpose is to simply tract the health factor of the loan. If the loan is in bad health, liquidators can
/// query the liquidation component to evaluate bad loans and liquidate the loan's collateral. Another purpose is to track
/// the status of the loan to update the user's credit report. In the future, there may be interesting use cases that
/// we can explore to securitize the loans or package them together.
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
        user_record: HashMap<NonFungibleId, User>,
        /// This is the resource address of the transient token that will be sent to the user management component.
        /// It is used to verify that the transient token that is sent to the user management component belongs to
        /// the pools of this protocol.
        allowed_resource: Vec<ResourceAddress>,
        /// The access vadge that is contained in this vault is used as a mechanism to allow the transient token to be
        /// burnt.
        access_vault: Vault,
    }

    /// Instantiates the User Management component. This is instantiated through the main router component. 
    /// At instantiation, the component requires the resource address of the authorization badge that is minted
    /// by the main router component. This logic simply says that only components with the access token is allowed
    /// to call the "register_resource" method. This method is used by the pools to register the transient tokens
    /// that are minted as a result of interacting with the pool (i.e to borrow). This is required to ensure there 
    /// are proper access controls to updating the User NFT. 
    impl UserManagement {
        pub fn new(allowed: ResourceAddress, access: Bucket) -> ComponentAddress {

            let access_rules: AccessRules = AccessRules::new().method("register_resource", rule!(require(allowed))).default(rule!(allow_all));

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
                        collateral_balance: HashMap::new(),
                        loans: BTreeSet::new(),
                        defaults: 0,
                        paid_off: 0,
                    },
                )
            });
            
            // Registers the user to the user_record HashMap
            {let user_id: NonFungibleId = user_nft.non_fungible::<User>().id();
                let user: User = user_nft.non_fungible().data();
                self.user_record.insert(user_id, user);}

            // Returns NFT to user
            return user_nft
        }

        /// Currently simply a way for other components to get the resource address of the NFT
        /// so that it can take informationa bout the NFT itself.
        pub fn get_nft(&self) -> ResourceAddress {
            return self.nft_address;
        }

        /// This method is used by the pool component to register the resource address of the transient
        /// token minted, so that the User Management component can check that the transient token passed
        /// to this component indeed belongs to the pool.
        pub fn register_resource(&mut self, resource_address: ResourceAddress) {
            self.allowed_resource.push(resource_address)
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
        fn check_lien(&self, user_id: &NonFungibleId, token_requested: &ResourceAddress) {
            // Check if deposit withdrawal request has no lien
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            return assert_eq!(nft_data.borrow_balance.get(&token_requested).unwrap_or(&Decimal::zero()), &Decimal::zero(), "User need to repay loan")
        }

        /// Adds the deposit balance
        /// Checks if the user already a record of the resource or not
        /// Requires a NonFungibleId so the method knows which NFT to update the data
        /// The lending pool deposit method mints a transient resource that contains the amount that has been deposited to the pool
        /// The transient resource address is then registered to this component where add_deposit_balance checks whether the transient resource token that has been passed
        /// Is the same as the transient resource that was created in the lending pool component
        /// The NFT data is then updated and the transient resource is burnt.
        pub fn add_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool.
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit.
            assert_eq!(amount, transient_token.amount(), "Incorrect amount.");

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            if nft_data.deposit_balance.contains_key(&address) {
                *nft_data.deposit_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                nft_data.deposit_balance.insert(address, amount);
            };

            // Burns the transient token
            self.access_vault.authorize(|| transient_token.burn());
            
            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        /// Check and understand the logic here - 06/01/2022
        pub fn decrease_deposit_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, redeem_amount: Decimal, transient_token: Bucket) {

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
            assert!(nft_data.deposit_balance.contains_key(&address), "Must have this deposit resource to withdraw");
            *nft_data.deposit_balance.get_mut(&address).unwrap() -= redeem_amount;

            assert!(nft_data.deposit_balance.get_mut(&address).unwrap() >= &mut Decimal::zero(), "Deposit balance cannot be negative.");

            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));

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
        pub fn add_borrow_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(amount, transient_token.amount(), "Incorrect amount.");

            // Asserts user exists
            self.assert_user_exist(&user_id);

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            // Need to figure out why the collateral ratio is not updated 06/09/22
            if nft_data.borrow_balance.contains_key(&address) {
                *nft_data.borrow_balance.get_mut(&address).unwrap_or(&mut Decimal::zero()) += amount;
            } else {
                nft_data.borrow_balance.insert(address, amount);
            };
            
            self.access_vault.authorize(|| transient_token.burn());

            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
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
        pub fn insert_loan(&mut self, user_id: NonFungibleId, loan_id: NonFungibleId) {

            // Asserts user exists
            self.assert_user_exist(&user_id);

            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            if nft_data.loans.contains(&loan_id) {
                info!("Loan has already been recorded.")
            } else {
                nft_data.loans.insert(loan_id);
            }
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

        pub fn convert_deposit_to_collateral(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(
                amount, transient_token.amount(),
                "Incorrect amount.");

            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            // Converts the deposit to collateral balance by subtracting from deposit and adding to collateral balance.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            if nft_data.collateral_balance.contains_key(&address) {
                *nft_data.deposit_balance.get_mut(&address).unwrap() -= amount;
                *nft_data.collateral_balance.get_mut(&address).unwrap() += amount;
            } else {
                *nft_data.deposit_balance.get_mut(&address).unwrap() -= amount;
                nft_data.collateral_balance.insert(address, amount);
            };

            self.access_vault.authorize(|| transient_token.burn());
            
            
            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        pub fn convert_collateral_to_deposit(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

            // Checks to see whether the transient token passed came from the lending pool
            assert!(
                self.allowed_resource.contains(&transient_token.resource_address()), 
                "Incorrect resource passed in for loan terms"
            );

            // Verify transient token is provided the same amount as the deposit
            assert_eq!(amount, transient_token.amount(), "Incorrect amount.");


            // If the User already has the resource address, adds to the balance. If not, registers new resource address.
            // Converts the deposit to collateral balance by subtracting from deposit and adding to collateral balance.
            let resource_manager = borrow_resource_manager!(self.nft_address);
            let mut nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            if nft_data.deposit_balance.contains_key(&address) {
                *nft_data.collateral_balance.get_mut(&address).unwrap() -= amount;
                *nft_data.deposit_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                *nft_data.collateral_balance.get_mut(&address).unwrap() -= amount;
                nft_data.deposit_balance.insert(address, amount);
            };

            self.access_vault.authorize(|| transient_token.burn());
            
            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        pub fn add_collateral_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, amount: Decimal, transient_token: Bucket) {

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

            if nft_data.collateral_balance.contains_key(&address) {
                *nft_data.collateral_balance.get_mut(&address).unwrap() += amount;
            }
            else {
                nft_data.collateral_balance.insert(address, amount);
            };

            self.access_vault.authorize(|| transient_token.burn());
            
            
            // Commits state
            self.user_badge_vault.authorize(|| resource_manager.update_non_fungible_data(&user_id, nft_data));
        }

        pub fn decrease_collateral_balance(&mut self, user_id: NonFungibleId, address: ResourceAddress, redeem_amount: Decimal, transient_token: Bucket) -> Decimal {

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
    }
}
