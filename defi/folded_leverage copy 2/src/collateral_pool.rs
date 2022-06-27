use scrypto::prelude::*;
use crate::user_management::*;
use crate::lending_pool::*;


// Still need to figure out how to calculate fees and interest rate
// Rational for NFT badge is to have a tracker and dashboard of loans, deposit, collateral, and user's risk profile.
// Advantages of having LP token as a separation to NFT badge is that it can be use for something else. 

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct User {
    #[scrypto(mutable)]
    deposit_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    collateral_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    borrow_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    collateral_ratio: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    loans: HashMap<ResourceAddress, Loan>,
}

blueprint! {
    struct CollateralPool {
        // Vault for lending pool
        collateral_vaults: HashMap<ResourceAddress, Vault>,
        // Badge for minting tracking tokens
        tracking_token_admin_badge: Vault,
        // Tracking tokens to be stored in borrowed_vaults whenever liquidity is removed from deposits
        tracking_token_address: ResourceAddress,
        // TBD
        fees: Vault,
        transient_vault: Vault,
        transient_token: ResourceAddress,
        user_management_address: ComponentAddress,
        access_vault: Vault,
        lending_pool: Option<ComponentAddress>,
    }

    impl CollateralPool {
        pub fn new(user_component_address: ComponentAddress, initial_funds: Bucket, access_badge: Bucket) -> (ComponentAddress, Bucket) {

            assert_ne!(
                borrow_resource_manager!(initial_funds.resource_address()).resource_type(), ResourceType::NonFungible,
                "[Pool Creation]: Asset must be fungible."
            );

            assert!(
                !initial_funds.is_empty(), 
                "[Pool Creation]: Can't deposit an empty bucket."
            ); 

            let user_management_address: ComponentAddress = user_component_address;

            // Define the resource address of the fees collected
            let funds_resource_def = initial_funds.resource_address();

            // Creating the admin badge of the liquidity pool which will be given the authority to mint and burn the
            // tracking tokens issued to the liquidity providers.
            let tracking_token_admin_badge: Bucket = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Tracking Token Admin Badge")
                .metadata("symbol", "TTAB")
                .metadata("description", "This is an admin badge that has the authority to mint and burn tracking tokens")
                .metadata("lp_id", format!("{}", initial_funds.resource_address()))
                .initial_supply(1);

            // Creating the tracking tokens and minting the amount owed to the initial liquidity provider
            let tracking_tokens: ResourceAddress = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata("name", format!("Borrowed Tracking Token"))
                .metadata("symbol", "TT")
                .metadata("description", "A tracking token used to track the percentage ownership of liquidity providers over the liquidity pool")
                .metadata("lp_id", format!("{}", initial_funds.resource_address()))
                .mintable(rule!(require(tracking_token_admin_badge.resource_address())), LOCKED)
                .burnable(rule!(require(tracking_token_admin_badge.resource_address())), LOCKED)
                .no_initial_supply();

            // Creates badge to authorizie to mint/burn transient token which is used as verification that the deposit/borrow/repay/redeem methods have been called
            let transient_token_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Admin authority for BasicFlashLoan")
                .initial_supply(1);

            let transient_token = ResourceBuilder::new_fungible()
                .metadata(
                    "name",
                    "Promise token - must be returned to be burned!",
                )
                .mintable(rule!(require(transient_token_badge.resource_address())), LOCKED)
                .burnable(rule!(require(access_badge.resource_address())), LOCKED)
                .restrict_deposit(rule!(deny_all), LOCKED)
                .initial_supply(initial_funds.amount());

            let transient_token_address = transient_token.resource_address();

            //Inserting pool info into HashMap
            let pool_resource_address = initial_funds.resource_address();
            let lending_pool: Bucket = initial_funds;
            let mut collateral_vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            collateral_vaults.insert(pool_resource_address, Vault::with_bucket(lending_pool));
            let mut transient_token_bucket = Bucket::new(transient_token.resource_address());
            transient_token_bucket.put(transient_token);

            //Instantiate lending pool component
            let collateral_pool: ComponentAddress = Self {
                collateral_vaults: collateral_vaults,
                tracking_token_address: tracking_tokens,
                tracking_token_admin_badge: Vault::with_bucket(tracking_token_admin_badge),
                fees: Vault::new(funds_resource_def),
                transient_vault: Vault::with_bucket(transient_token_badge),
                transient_token: transient_token_address,
                user_management_address: user_management_address,
                access_vault: Vault::with_bucket(access_badge),
                lending_pool: None,
            }
            .instantiate().globalize();
            return (collateral_pool, transient_token_bucket);
        }

        pub fn set_address(&mut self, lending_pool_address: ComponentAddress) {
            self.lending_pool.get_or_insert(lending_pool_address);
        }

        // This method is also being used in the lending pool component as a convertion from deposit supply to collateral supply
        // Is it important to distinguish between regular supply and conversions?
        pub fn deposit(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_amount: Bucket) {
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management_address.into();

            let dec_deposit_amount = deposit_amount.amount();

            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(dec_deposit_amount)});

            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});
            user_management.add_collateral_balance(user_id, token_address, dec_deposit_amount, transient_token);
            // Deposits collateral into the vault
            self.collateral_vaults.get_mut(&deposit_amount.resource_address()).unwrap().put(deposit_amount);
        }

        pub fn convert_from_deposit(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, collateral_amount: Bucket) {
            assert_eq!(token_address, collateral_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management_address.into();

            let dec_collateral_amount = collateral_amount.amount();

            // Creates a transient resource to be passed to UserManagement Component
            // Ensures that transient resources came from the Collateral Pool Component
            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(dec_collateral_amount)});

            // Lets the User Management Component know that the transient resource has been created from the Collateral Pool Component
            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});
            // Calls User Management method to modify NFT data
            // Additional checks in the User Management
            user_management.convert_deposit_to_collateral(user_id, token_address, dec_collateral_amount, transient_token);
            // Deposits collateral into the vault
            self.collateral_vaults.get_mut(&collateral_amount.resource_address()).unwrap().put(collateral_amount);
        }

        /// Gets the resource addresses of the tokens in this liquidity pool and returns them as a `Vec<ResourceAddress>`.
        /// 
        /// # Returns:
        /// 
        /// `Vec<ResourceAddress>` - A vector of the resource addresses of the tokens in this liquidity pool.
        pub fn addresses(&self) -> Vec<ResourceAddress> {
            return self.collateral_vaults.keys().cloned().collect::<Vec<ResourceAddress>>();
        }

        pub fn belongs_to_pool(
            &self, 
            address: ResourceAddress
        ) -> bool {
            return self.collateral_vaults.contains_key(&address);
        }

        pub fn assert_belongs_to_pool(
            &self, 
            address: ResourceAddress, 
            label: String
        ) {
            assert!(
                self.belongs_to_pool(address), 
                "[{}]: The provided resource address does not belong to the pool.", 
                label
            );
        }

        fn withdraw(&mut self, resource_address: ResourceAddress, amount: Decimal) -> Bucket {
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

        pub fn convert_to_deposit(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_amount: Decimal) {

            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management_address.into();

            // Gets the user badge ResourceAddress
            let nft_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(nft_resource);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);

            // Check if the user has enough collateral supply to convert to deposit supply
            assert!(*nft_data.collateral_balance.get(&token_address).unwrap() >= deposit_amount, "Must have enough deposit supply to use as a collateral");

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], self.collateral_vaults[&addresses[0]].amount() - deposit_amount);
            let lending_pool: LendingPool = self.lending_pool.unwrap().into();
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
        pub fn redeem(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, redeem_amount: Decimal) -> Bucket {

            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management_address.into();

            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(redeem_amount)});

            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});

            // Reduce deposit balance of the user
            user_management.decrease_deposit_balance(user_id, token_address, redeem_amount, transient_token);

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], self.collateral_vaults[&addresses[0]].amount() - redeem_amount);
            return bucket;
        }

        pub fn check_total_supplied(&self, token_address: ResourceAddress) -> Decimal {
            let vault = self.collateral_vaults.get(&token_address).unwrap();
            return vault.amount()
        }
        
    }
}


