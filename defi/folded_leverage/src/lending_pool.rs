use scrypto::prelude::*;
use crate::user_management::*;
use crate::collateral_pool::*;

#[derive(TypeId, Encode, Decode, Describe)]
pub enum Status {
    PaidOff,
    Defaulted,
    Current,
}

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct User {
    #[scrypto(mutable)]
    deposit_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    collateral_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    borrow_balance: HashMap<ResourceAddress, Decimal>,
    #[scrypto(mutable)]
    loans: BTreeSet<NonFungibleId>,
    defaults: u64,
    paid_off: u64,
}

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct Loan {
    asset: ResourceAddress,
    collateral: ResourceAddress,
    owner: NonFungibleId,
    #[scrypto(mutable)]
    loan_amount: Decimal,
    collateral_amount: Decimal,
    collateral_ratio: Decimal,
    loan_status: Status,
}

blueprint! {
    /// This is a struct used to define the Lending Pool.
    struct LendingPool {
        /// This is the vaults where the reserves of the tokens will be stored. The choice of storing the two
        /// vaults in a hashmap instead of storing them in two different struct variables is made to allow for an easier
        /// and more dynamic way of manipulating and making use of the vaults inside the liquidity pool. 
        vaults: HashMap<ResourceAddress, Vault>,
        /// This is the vault that tracks the amounts that have been borrowed from this lending pool. Any time
        /// a user borrows from the lending pool, there will be tokens minted of the amount taken from the lending pool
        /// that is deposited to this vault. Its purpose is to simply track the amount that has been borrowed and repaid. 
        borrowed_vaults: HashMap<ResourceAddress, Vault>,
        /// Badge for minting tracking tokens that track the amounts borrowed from this lending pool.
        tracking_token_admin_badge: Vault,
        /// Tracking tokens to be stored in borrowed_vaults whenever liquidity is removed from deposits. Tracking tokens
        /// will also be burnt when borrowed amounts have been repaid.
        tracking_token_address: ResourceAddress,
        /// Unsure yet how the fee design will work. One option is to deposit all the fees into a vault. Another option is 
        /// the fee will be deposited into the lending pool where there won't be much logic that may need to be implemented
        /// to claim fees. Still need to be discussed.
        fee_vault: Vault,
        /// Temporary static fee as of now. 
        borrow_fees: Decimal,
        /// This vault is used to create transient tokens. The purpose of transient tokens are simply for authorization and checks.
        /// Transient tokens are used to make sure no one can just access the User Management component
        /// Only way to change NFT data is by calling methods from the pool that would give cause to 
        /// to changing the NFT data i.e depositing, borrowing, etc.
        /// Pool methods will create a transient token and calls a protected method from the User Management compononet
        /// to register the resource address of the transient token
        /// User Management component is now aware that a transient token from the Pool has been created
        /// The transient token passed to the User Management component has to be the same one created from the Pool(s).
        transient_vault: Vault,
        transient_token: ResourceAddress,
        user_management_address: ComponentAddress,
        /// Access badge to call permissioned method from the UserManagement component.
        access_vault: Vault,
        max_borrow: Decimal,
        min_collateral_ratio: Decimal,
        /// It's meant to retrieve the NFT resource of the User, but will be TBD for now.
        nft_address: Vec<ResourceAddress>,
        nft_id: Vec<NonFungibleId>,
        /// The Lending Pool is a shared pool which shares resources with the collateral pool. This allows the lending pool to access methods from the collateral pool.
        collateral_pool: Option<ComponentAddress>,
        /// This badge creates the NFT loan. Perhaps consolidate the admin badges?
        loan_issuer_badge: Vault,
        /// NFT loans are minted to create documentations of the loan that has been issued and repaid along with the loan terms.
        loan_vault: Vault,
        /// Allows for lending pool to access methods from liquidation component.
        liquidation_component: Option<ComponentAddress>,
        /// Creates a list of Loan NFTs are bad so users can query and sort through.
        bad_loans: BTreeSet<NonFungibleId>,
    }

    impl LendingPool {
        /// Instantiates the lending pool.
        /// 
        /// # Description:
        /// This method instantiates the lending pool where people can supply deposits, borrow, repay, or redeem from 
        /// the pool. There will be a simple origination fee for every borrow requested with an additional simple interest
        /// charged to borrow from this pool. 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        /// 
        /// * **Check 1:** Checks to ensure that the initial_funds are fungibles.
        /// * **Check 2:** Checks to ensure that the initial_funds bucket is not empty.
        /// 
        /// # Arguments:
        /// 
        /// * `user_component_address` (ComponentAddress) - This is the component address of the User Management component. It 
        /// allows the lending pool to access methods from the User Management component in order to update the User NFT.
        /// 
        /// * `initial_funds` (Bucket) - This provides the initial liquidity for the lending pool.
        /// 
        /// * `access_badge` (Bucket) - This is the access badge that allows the lending pool to call a permissioned method from
        /// the User Management component called the `register_resource` method which registers the transient token minted from
        /// this pool.
        /// 
        /// # Returns:
        /// 
        /// * `ComponentAddress` - The ComponentAddress of the newly created LendingPool.
        /// * `Bucket` - The transient token minted.
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

            // Badge that will be stored in the component's vault to update loan NFT.
            let loan_issuer_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("user", "Loan Issuer Badge")
                .initial_supply(1);
        
            // NFT description for loan information
            let loan_nft_address: ResourceAddress = ResourceBuilder::new_non_fungible()
                .metadata("user", "Loan NFT")
                .mintable(rule!(require(loan_issuer_badge.resource_address())), LOCKED)
                .burnable(rule!(require(loan_issuer_badge.resource_address())), LOCKED)
                .updateable_non_fungible_data(rule!(require(loan_issuer_badge.resource_address())), LOCKED)
                .no_initial_supply();

            //Inserting pool info into HashMap
            let pool_resource_address = initial_funds.resource_address();
            let lending_pool: Bucket = initial_funds;
            let mut vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            let mut borrowed_vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            vaults.insert(pool_resource_address, Vault::with_bucket(lending_pool));
            borrowed_vaults.insert(pool_resource_address, Vault::new(tracking_tokens));
            let mut transient_token_bucket = Bucket::new(transient_token.resource_address());
            transient_token_bucket.put(transient_token);

            //Instantiate lending pool component
            let lending_pool: ComponentAddress = Self {
                vaults: vaults,
                borrowed_vaults: borrowed_vaults,
                tracking_token_address: tracking_tokens,
                tracking_token_admin_badge: Vault::with_bucket(tracking_token_admin_badge),
                fee_vault: Vault::new(funds_resource_def),
                borrow_fees: dec!("1.0"),
                transient_vault: Vault::with_bucket(transient_token_badge),
                transient_token: transient_token_address,
                user_management_address: user_management_address,
                access_vault: Vault::with_bucket(access_badge),
                max_borrow: dec!("0.5"),
                min_collateral_ratio: dec!("1.0"),
                nft_address: Vec::new(),
                nft_id: Vec::new(),
                collateral_pool: None,
                loan_issuer_badge: Vault::with_bucket(loan_issuer_badge),
                loan_vault: Vault::new(loan_nft_address),
                liquidation_component: None,
                bad_loans: BTreeSet::new(),
            }
            .instantiate().globalize();
            return (lending_pool, transient_token_bucket);
        }

        pub fn loan_nft(&self) -> ResourceAddress {
            return self.loan_vault.resource_address();
        }

        pub fn set_address(&mut self, collateral_pool_address: ComponentAddress) {
            self.collateral_pool.get_or_insert(collateral_pool_address);
        }

        pub fn set_liquidation_address(&mut self, component_address: ComponentAddress) {
            self.liquidation_component.get_or_insert(component_address);
        }

        // Mint tracking tokens every time there's a borrow and puts it in the borrowed vault
        fn mint_borrow(&mut self, token_address: ResourceAddress, amount: Decimal) {
            let tracking_tokens_manager: &ResourceManager = borrow_resource_manager!(self.tracking_token_address);
            let tracking_tokens: Bucket = self.tracking_token_admin_badge.authorize(|| {tracking_tokens_manager.mint(amount)});
            self.borrowed_vaults.get_mut(&token_address).unwrap().put(tracking_tokens)
        }

        // Burn tracking tokens every time there's a repayment
        fn burn_borrow(&mut self, token_address: ResourceAddress, amount: Decimal) {
            let burn_amount: Bucket = self.borrowed_vaults.get_mut(&token_address).unwrap().take(amount);
            let tracking_tokens_manager: &ResourceManager = borrow_resource_manager!(self.tracking_token_address);
            self.tracking_token_admin_badge.authorize(|| {tracking_tokens_manager.burn(burn_amount)});
        }

        pub fn register_user(&mut self, nft_resource_address: ResourceAddress) {
            self.nft_address.push(nft_resource_address);
        }

        pub fn register_user_id(&mut self, nft_id: NonFungibleId) {
            self.nft_id.push(nft_id);
        }

        /// Deposits supply into the lending pool.
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a few checks before the borrow balance increases.
        /// 
        /// * **Check 1:** Checks to ensure that the token selected to be depsoited is the same as the tokens sent.
        /// * **Check 2:** Checks to ensure that the deposit bucket is not empty.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the supply deposit.
        /// 
        /// * `deposit_amount` (Bucket) - This is the bucket that contains the deposit supply.
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned.
        pub fn deposit(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_amount: Bucket) {
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");

            assert!(
                !deposit_amount.is_empty(), 
                "[Pool Creation]: Can't deposit an empty bucket."
            ); 
            
            let user_management: UserManagement = self.user_management_address.into();

            let dec_deposit_amount = deposit_amount.amount();

            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(dec_deposit_amount)});

            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});
            user_management.add_deposit_balance(user_id, token_address, dec_deposit_amount, transient_token);

            // Deposits collateral
            self.vaults.get_mut(&deposit_amount.resource_address()).unwrap().put(deposit_amount);
        }

        /// Converts the collateral back to supply deposit.
        /// 
        /// # Description:
        /// This method is used in the event that the user may change their mind of using their deposit supply as collateral 
        /// (which will become locked/illiquid) or if the loan has been paid off with the remaining collateral to be used
        /// as supply liquidity and earn rewards. This method is called first from the router component which is routed
        /// to the correct collateral pool.
        /// 
        /// This method currently has no checks.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested collateral to be converted back to supply.
        /// 
        /// * `deposit_amount` (Bucket) - This is the bucket that contains the deposit supply.
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned.
        /// 
        /// # Design questions: 
        /// * Should this method be protected that only Collateral Component can call? 06/11/22
        /// * Currently, anyone can essentially deposit.
        pub fn convert_from_collateral(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_amount: Bucket) {
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");
            
            let user_management: UserManagement = self.user_management_address.into();

            let dec_deposit_amount = deposit_amount.amount();

            // Creates a transient token that confirms the conversion from collateral to deposit.
            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(dec_deposit_amount)
            });

            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});
            user_management.convert_collateral_to_deposit(user_id, token_address, dec_deposit_amount, transient_token);
            // Deposits collateral
            self.vaults.get_mut(&deposit_amount.resource_address()).unwrap().put(deposit_amount);
        }

        /// Gets the resource addresses of the tokens in this liquidity pool and returns them as a `Vec<ResourceAddress>`.
        /// 
        /// # Returns:
        /// 
        /// `Vec<ResourceAddress>` - A vector of the resource addresses of the tokens in this liquidity pool.
        pub fn addresses(&self) -> Vec<ResourceAddress> {
            return self.vaults.keys().cloned().collect::<Vec<ResourceAddress>>();
        }

        /// Checks if the given address belongs to this pool or not.
        /// 
        /// This method is used to check if a given resource address belongs to the token in this lending pool
        /// or not. A resource belongs to a lending pool if its address is in the addresses in the `vaults` HashMap.
        /// 
        /// # Arguments:
        /// 
        /// * `address` (ResourceAddress) - The address of the resource that we wish to check if it belongs to the pool.
        /// 
        /// # Returns:
        /// 
        /// * `bool` - A boolean of whether the address belongs to this pool or not.
        pub fn belongs_to_pool(
            &self, 
            address: ResourceAddress
        ) -> bool {
            return self.vaults.contains_key(&address);
        }

        /// Asserts that the given address belongs to the pool.
        /// 
        /// This is a quick assert method that checks if a given address belongs to the pool or not. If the address does
        /// not belong to the pool, then an assertion error (panic) occurs and the message given is outputted.
        /// 
        /// # Arguments:
        /// 
        /// * `address` (ResourceAddress) - The address of the resource that we wish to check if it belongs to the pool.
        /// * `label` (String) - The label of the method that called this assert method. As an example, if the swap 
        /// method were to call this method, then the label would be `Swap` so that it's clear where the assertion error
        /// took place.
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

        /// Withdraws tokens from the lending pool.
        /// 
        /// This method is used to withdraw a specific amount of tokens from the lending pool. 
        /// 
        /// This method performs a number of checks before the withdraw is made:
        /// 
        /// * **Check 1:** Checks that the resource address given does indeed belong to this lending pool.
        /// * **Check 2:** Checks that the there is enough liquidity to perform the withdraw.
        /// 
        /// # Arguments:
        /// 
        /// * `resource_address` (ResourceAddress) - The address of the resource to withdraw from the liquidity pool.
        /// * `amount` (Decimal) - The amount of tokens to withdraw from the liquidity pool.
        /// 
        /// # Returns:
        /// 
        /// * `Bucket` - A bucket of the withdrawn tokens.
        fn withdraw(&mut self, resource_address: ResourceAddress, amount: Decimal) -> Bucket {
            // Performing the checks to ensure that the withdraw can actually go through
            self.assert_belongs_to_pool(resource_address, String::from("Withdraw"));
            
            // Getting the vault of that resource and checking if there is enough liquidity to perform the withdraw.
            let vault: &mut Vault = self.vaults.get_mut(&resource_address).unwrap();
            assert!(
                vault.amount() >= amount,
                "[Withdraw]: Not enough liquidity available for the withdraw."
            );
            

            return vault.take(amount);
        }

        /// Allows user to borrow funds from the pool.
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a number of checks before the borrow is made:
        /// 
        /// * **Check 1:** Checks that the borrow amount must be less than or equals to 50% of your collateral. Which is
        /// currently the simple default borrow amount.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested collateral to be converted back to supply.
        /// 
        /// * `deposit_amount` (Bucket) - This is the bucket that contains the deposit supply.
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned.
        /// 
        /// # Design questions: 
        /// * Should this method be protected that only Collateral Component can call? 06/11/22
        /// * Currently, anyone can essentially deposit.
        pub fn borrow(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, borrow_amount: Decimal, borrow_fee: Bucket) -> Bucket {

            let user_management: UserManagement = self.user_management_address.into();
            let nft_resource = user_management.get_nft();

            // Check borrow percent
            let resource_manager = borrow_resource_manager!(nft_resource);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            // It's unwrap because if the user does not have collateral, it will panic.
            let collateral_amount = *nft_data.collateral_balance.get(&token_address).unwrap();
            assert!(borrow_amount <= collateral_amount * self.max_borrow, "Borrow amount must be less than or equals to 50% of your collateral.");

            // Calculate fees charged
            let fee = self.borrow_fees;
            let fee_charged = borrow_amount * fee;
            assert_eq!(fee_charged, borrow_fee.amount(), "Not enough fee provided to borrow");
            assert!(!borrow_fee.is_empty(), "Must pay fees!");
            self.fee_vault.take(borrow_fee.amount());
            
            // Mints the transient tokens to be sent to the User Management component to ensure that the borrow method from the lending
            // pool has been called.
            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(borrow_amount)});

            // Permissioned call to the User Management component to register that the transient token passed belongs to this pool.
            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});

            // Commits state
            user_management.add_borrow_balance(user_id.clone(), token_address, borrow_amount, transient_token);

            // Minting tracking tokens to be deposited to borrowed_vault to track borrows from this pool
            self.mint_borrow(token_address, borrow_amount);

            // Mint loan NFT
            let loan_nft = self.loan_issuer_badge.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.loan_vault.resource_address());
                resource_manager.mint_non_fungible(
                    // The User id
                    &NonFungibleId::random(),
                    // The User data
                    Loan {
                        asset: token_address,
                        collateral: token_address,
                        owner: user_id.clone(),
                        loan_amount: borrow_amount,
                        collateral_amount: collateral_amount,
                        collateral_ratio: ( collateral_amount / borrow_amount ),
                        loan_status: Status::Current,
                    },
                )
            });

            let loan_nft_id = loan_nft.non_fungible::<Loan>().id();

            // Insert Loan NFT to the User NFT
            user_management.insert_loan(user_id.clone(), loan_nft_id);


            self.loan_vault.put(loan_nft);

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let borrow_amount: Bucket = self.withdraw(addresses[0], self.vaults[&addresses[0]].amount() - borrow_amount);

            return borrow_amount
        }

        // We don't want anyone to just be able to convert the deposit to collateral
        // Have to find a way to tie whose deposit is whose
        // NFT user badge should be able to do this
        // Transient token not needed. Only check is to make sure that the deposit the User is trying to convert is the User's
        pub fn convert_to_collateral(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_collateral: Decimal) {

            let pool_resource_address = self.vaults.contains_key(&token_address);
            assert!(pool_resource_address == true, "Requested asset must be the same as the lending pool.");

            let user_management: UserManagement = self.user_management_address.into();      

            // Gets the user badge ResourceAddress
            let nft_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(nft_resource);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);


            // Check if the user has enough deposit supply to convert to collateral supply
            assert!(*nft_data.deposit_balance.get(&token_address).unwrap() >= deposit_collateral, "Must have enough deposit supply to use as a collateral");

            let addresses: Vec<ResourceAddress> = self.addresses();
            // Creating a bucket to remove deposit supply from the lending pool to transfer to collateral pool
            let collateral_amount: Bucket = self.withdraw(addresses[0], self.vaults[&addresses[0]].amount() - deposit_collateral);
            let collateral_pool: CollateralPool = self.collateral_pool.unwrap().into();
            collateral_pool.convert_from_deposit(user_id, token_address, collateral_amount);
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
            let bucket: Bucket = self.withdraw(addresses[0], self.vaults[&addresses[0]].amount() - redeem_amount);
            return bucket;
        }
        
        pub fn repay(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, mut repay_amount: Bucket) -> Bucket {
            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management_address.into();

            let dec_repay_amount = repay_amount.amount();
            let transient_token = self.transient_vault.authorize(|| {
                borrow_resource_manager!(self.transient_token).mint(dec_repay_amount)});

            self.access_vault.authorize(|| {user_management.register_resource(transient_token.resource_address())});

            // Burns the tracking token for borrowed amounts
            let amount = repay_amount.amount();
            self.burn_borrow(token_address, amount);

            // Update Loan NFT
            
            // Commits state
            // Need to fix this
            let to_return_amount = user_management.decrease_borrow_balance(user_id, token_address, dec_repay_amount, transient_token);
            let to_return = repay_amount.take(to_return_amount);

            // Deposits the repaid loan back into the supply
            self.vaults.get_mut(&repay_amount.resource_address()).unwrap().put(repay_amount);
            to_return
        }


        
        /// Finds loans that are below the minimum collateral ratio allowed. 
        /// 
        /// # Description:
        /// 
        /// 
        /// 
        /// This method performs a number of checks before the borrow is made:
        /// 
        /// * **Check 1:** Checks that the borrow amount must be less than or equals to 50% of your collateral. Which is
        /// currently the simple default borrow amount.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested collateral to be converted back to supply.
        /// 
        /// * `deposit_amount` (Bucket) - This is the bucket that contains the deposit supply.
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned.
        /// 
        /// # Design questions: 
        /// * Should this method be protected that only Collateral Component can call? 06/11/22
        /// * Currently, anyone can essentially deposit.
        // Have to test whether this actually works or not
        fn find_bad_loans(&mut self) {
            let loans = self.loan_vault.non_fungible_ids();
            let mut check_loans = loans.iter();
            let next_loan = check_loans.next().unwrap().clone();
            for next_loan in check_loans {
                let get_collateral_ratio = self.check_loan_nft(&next_loan);
                if get_collateral_ratio < self.min_collateral_ratio {
                    self.bad_loans.insert(next_loan.clone());
                }
            };
        }

        // Definitely need some authorization here
        pub fn transfer_bad_loan(&mut self, loan_id: NonFungibleId) -> Bucket {
            let bad_loan: Bucket = self.loan_vault.take_non_fungible(&loan_id);
            return bad_loan;
        }

        pub fn get_loan_resource(&self) -> ResourceAddress {
            return self.loan_vault.resource_address();
        }

        pub fn check_collateral_ratio(&self, loan_id: NonFungibleId) -> bool {
            let resource_manager = borrow_resource_manager!(self.loan_vault.resource_address());
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            let collateral_ratio = loan_data.collateral_ratio;
            let boolean: bool = collateral_ratio < self.min_collateral_ratio;
            return boolean;
        }

        pub fn get_bad_loans(&mut self) -> String {
            // Pushes bad loans to the struct
            self.find_bad_loans();
            let mut view_bad_loans = self.bad_loans.iter();
            let next_loan = view_bad_loans.next().unwrap();
            let mut string = String::new();
            for next_loan in view_bad_loans {
                let retrieve_loan_info = self.check_loan_nft(next_loan);
                let loan_str = format!("{}-{}", next_loan, retrieve_loan_info);
                string.push_str(&loan_str);
            };

            return string;
        }

        fn check_loan_nft(&self, loan_id: &NonFungibleId) -> Decimal {
            let resource_manager = borrow_resource_manager!(self.loan_vault.resource_address());
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            return loan_data.collateral_ratio;
        }

        // Refactor to utils at some point
        // Math is off (or maybe decimal doesn't have negative number?) 06/01/22
        pub fn check_liquidity(&mut self, token_address: ResourceAddress) -> Decimal {
            let vault: &mut Vault = self.vaults.get_mut(&token_address).unwrap();
            let borrowed_vault: &mut Vault = self.borrowed_vaults.get_mut(&token_address).unwrap();
            let liquidity_amount: Decimal = borrowed_vault.amount() - vault.amount();
            return liquidity_amount
        }

        pub fn check_utilization_rate(&mut self, token_address: ResourceAddress) -> Decimal {
            let vault: &mut Vault = self.vaults.get_mut(&token_address).unwrap();
            let borrowed_vault: &mut Vault = self.borrowed_vaults.get_mut(&token_address).unwrap();
            let liquidity_amount: Decimal = borrowed_vault.amount() / vault.amount();
            return liquidity_amount
        }

        pub fn check_total_supplied(&self, token_address: ResourceAddress) -> Decimal {
            let vault = self.vaults.get(&token_address).unwrap();
            return vault.amount()
        }
        
        pub fn check_total_borrowed(&self, token_address: ResourceAddress) -> Decimal {
            let borrowed_vault = self.borrowed_vaults.get(&token_address).unwrap();
            return borrowed_vault.amount()
        }
        
    }
}


