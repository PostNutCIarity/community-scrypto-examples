use scrypto::prelude::*;
use crate::user_management::*;
use crate::lending_pool::*;
use crate::structs::{User, Loan, Status};

blueprint! {
    struct CollateralPool {
        // Vault for lending pool
        collateral_vaults: HashMap<ResourceAddress, Vault>,
        user_management: ComponentAddress,
        access_vault: Vault,
        lending_pool: ComponentAddress,
    }

    impl CollateralPool {
        pub fn new(
            user_component_address: ComponentAddress,
            lending_pool_address: ComponentAddress,
            token_address: ResourceAddress,
            access_badge: Bucket
        ) -> ComponentAddress 
        {
            let access_rules: AccessRules = AccessRules::new()
            .method("withdraw_vault", rule!(require(access_badge.resource_address())))
            .default(rule!(allow_all));

            assert_ne!(
                borrow_resource_manager!(token_address).resource_type(), ResourceType::NonFungible,
                "[Pool Creation]: Asset must be fungible."
            );

            let user_management_address: ComponentAddress = user_component_address;
            let lending_pool_address: ComponentAddress = lending_pool_address;

            //Inserting pool info into HashMap
            let mut collateral_vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            collateral_vaults.insert(token_address, Vault::new(token_address));

            //Instantiate lending pool component
            let collateral_pool: ComponentAddress = Self {
                collateral_vaults: collateral_vaults,
                user_management: user_management_address,
                access_vault: Vault::with_bucket(access_badge),
                lending_pool: lending_pool_address,
            }
            .instantiate()
            .add_access_check(access_rules)
            .globalize();
            return collateral_pool
        }

        // This method is also being used in the lending pool component as a convertion from deposit supply to collateral supply
        // Is it important to distinguish between regular supply and conversions?
        pub fn deposit(
            &mut self,
            user_id: NonFungibleId,
            token_address: ResourceAddress,
            deposit_amount: Bucket
        ) 
        {
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management.into();

            let dec_deposit_amount = deposit_amount.amount();

            self.access_vault.authorize(|| {user_management.add_collateral_balance(user_id, token_address, dec_deposit_amount);});

            // Deposits collateral into the vault
            self.collateral_vaults.get_mut(&deposit_amount.resource_address()).unwrap().put(deposit_amount);
        }

        pub fn deposit_additional(
            &mut self,
            user_id: NonFungibleId,
            loan_id: NonFungibleId,
            token_address: ResourceAddress,
            deposit_amount: Bucket
        ) 
        {
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management.into();
            
            let lending_pool: LendingPool = self.lending_pool.into();

            // Finds the loan NFT
            let loan_nft_resource = lending_pool.loan_nft();
            let resource_manager = borrow_resource_manager!(loan_nft_resource);
            let mut loan_nft_data: Loan = resource_manager.get_non_fungible_data(&loan_id);

            let dec_deposit_amount = deposit_amount.amount();

            // Updates the states
            self.access_vault.authorize(|| {user_management.add_collateral_balance(user_id, token_address, dec_deposit_amount);});
            loan_nft_data.collateral_amount += dec_deposit_amount;

            // Deposits collateral into the vault
            self.collateral_vaults.get_mut(&deposit_amount.resource_address()).unwrap().put(deposit_amount);
        }

        pub fn convert_from_deposit(
            &mut self,
            user_id: NonFungibleId,
            token_address: ResourceAddress,
            collateral_amount: Bucket
        ) 
        {
            assert_eq!(token_address, collateral_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management.into();

            let dec_collateral_amount = collateral_amount.amount();

            self.access_vault.authorize(|| {user_management.convert_deposit_to_collateral(user_id, token_address, dec_collateral_amount)});
            // Deposits collateral into the vault
            self.collateral_vaults.get_mut(&collateral_amount.resource_address()).unwrap().put(collateral_amount);
        }

        /// Gets the resource addresses of the tokens in this liquidity pool and returns them as a `Vec<ResourceAddress>`.
        /// 
        /// # Returns:
        /// 
        /// `Vec<ResourceAddress>` - A vector of the resource addresses of the tokens in this liquidity pool.
        pub fn addresses(
            &self
        ) -> Vec<ResourceAddress> 
        {
            return self.collateral_vaults.keys().cloned().collect::<Vec<ResourceAddress>>();
        }

        pub fn belongs_to_pool(
            &self, 
            address: ResourceAddress
        ) -> bool
        {
            return self.collateral_vaults.contains_key(&address);
        }

        pub fn assert_belongs_to_pool(
            &self, 
            address: ResourceAddress, 
            label: String
        ) 
        {
            assert!(
                self.belongs_to_pool(address), 
                "[{}]: The provided resource address does not belong to the pool.", 
                label
            );
        }

        fn withdraw(
            &mut self,
            resource_address: ResourceAddress,
            amount: Decimal
        ) -> Bucket 
        {
            // Performing the checks to ensure tha the withdraw can actually go through
            self.assert_belongs_to_pool(resource_address, String::from("Withdraw"));
            
            // Getting the vault of that resource and checking if there is enough liquidity to perform the withdraw.
            let vault: &mut Vault = self.collateral_vaults.get_mut(&resource_address).unwrap();
            assert!(
                vault.amount() >= amount,
                "[Withdraw]: Not enough liquidity available for the withdraw."
            );
            

            return vault.take(amount);
        }

        pub fn convert_to_deposit(
            &mut self, 
            user_id: NonFungibleId, 
            token_address: ResourceAddress, 
            deposit_amount: Decimal
        ) 
        {
            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management.into();

            // Gets the user badge ResourceAddress
            let nft_resource = user_management.get_sbt();
            let resource_manager = borrow_resource_manager!(nft_resource);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            let user_loans = nft_data.open_loans.iter();

            {
                // Looping through loans in the User SBT
                for (_token_address, loans) in user_loans {
                    let lending_pool: LendingPool = self.lending_pool.into();
                    let loan_resource = lending_pool.loan_nft();
                    let resource_manager = borrow_resource_manager!(loan_resource);
                    // Retrieve loan data for every loans in the User SBT
                    let loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                    let loan_status = loan_data.loan_status;
                    match loan_status {
                        Status::Current => assert!(loan_status != Status::Current, "Cannot have outstanding loans"),
                        _ => break,
                    }
                }
            }
            

            // Check if the user has enough collateral supply to convert to deposit supply
            assert!(*nft_data.collateral_balance.get(&token_address).unwrap() >= deposit_amount, "Must have enough deposit supply to use as a collateral");

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], deposit_amount);
            let lending_pool: LendingPool = self.lending_pool.into();
            lending_pool.convert_from_collateral(user_id, token_address, bucket);
        }

        /// Removes the percentage of the liquidity owed to this liquidity provider.
        /// 
        /// This method is used to calculate the amount of tokens owed to the liquidity provider and take them out of
        /// the liquidity pool and return them to the liquidity provider. If the liquidity provider wishes to only take
        /// out a portion of their liquidity instead of their total liquidity they can provide a `tracking_tokens` 
        /// bucket that does not contain all of their tracking tokens (example: if they want to withdraw 50% of their
        /// liquidity, they can put 50% of their tracking tokens into the `tracking_tokens` bucket.). When the liquidity
        /// provider is given the tokens that they are owed, the tracking tokens are burned.
        /// 
        /// This method performs a number of checks before liquidity removed from the pool:
        /// 
        /// * **Check 1:** Checks to ensure that the tracking tokens passed do indeed belong to this liquidity pool.
        /// 
        /// # Arguments:
        /// 
        /// * `tracking_tokens` (Bucket) - A bucket of the tracking tokens that the liquidity provider wishes to 
        /// exchange for their share of the liquidity.
        /// 
        /// # Returns:
        /// 
        /// * `Bucket` - A Bucket of the share of the liquidity provider of the first token.
        /// * `Bucket` - A Bucket of the share of the liquidity provider of the second token.
        pub fn redeem(
            &mut self, 
            user_id: NonFungibleId, 
            token_address: ResourceAddress, 
            redeem_amount: Decimal
        ) -> Bucket 
        {
            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management.into();
            let sbt_resource = user_management.get_sbt();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            let user_loans = sbt_data.open_loans.iter();

            let lending_pool: LendingPool = self.lending_pool.into();
            let loan_id = lending_pool.loan_nft();

            for (_token_address, loans) in user_loans {
                let resource_manager = borrow_resource_manager!(loan_id);
                let loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let check_paid_off = loan_data.loan_status;
                assert!(check_paid_off != Status::Current, "Must pay off loans before redeeming.");
            }

            // Reduce deposit balance of the user
            self.access_vault.authorize(|| {user_management.decrease_deposit_balance(user_id, token_address, redeem_amount)});

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], redeem_amount);
            return bucket;
        }

        pub fn check_total_collateral_supplied(
            &self, 
            token_address: ResourceAddress
        ) -> Decimal 
        {
            let vault = self.collateral_vaults.get(&token_address).unwrap();
            info!("The total collateral supplied in this pool is {:?}", vault.amount());
            return vault.amount()
        }

        pub fn withdraw_vault(
            &mut self, 
            amount: Decimal
        ) -> Bucket
        {
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], amount);
            return bucket
        }
    }
}


