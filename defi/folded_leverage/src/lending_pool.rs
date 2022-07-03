use scrypto::prelude::*;
use crate::user_management::*;
use crate::collateral_pool::*;

#[derive(TypeId, Encode, Decode, Describe, PartialEq)]
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
    #[scrypto(mutable)]
    defaults: u64,
    #[scrypto(mutable)]
    paid_off: u64,
}

#[derive(NonFungibleData, Describe, Encode, Decode, TypeId)]
pub struct Loan {
    asset: ResourceAddress,
    collateral: ResourceAddress,
    principal_loan_amount: Decimal,
    interest: Decimal,
    owner: NonFungibleId,
    #[scrypto(mutable)]
    remaining_balance: Decimal,
    #[scrypto(mutable)]
    interest_expense: Decimal,
    #[scrypto(mutable)]
    collateral_amount: Decimal,
    #[scrypto(mutable)]
    collateral_amount_usd: Decimal,
    #[scrypto(mutable)]
    health_factor: Decimal,
    #[scrypto(mutable)]
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
        interest: Decimal,
        xrd_usd: Decimal,
        user_management_address: ComponentAddress,
        /// Access badge to call permissioned method from the UserManagement component.
        access_vault: Vault,
        max_borrow: Decimal,
        min_health_factor: Decimal,
        /// It's meant to retrieve the NFT resource of the User, but will be TBD for now.
        nft_address: Vec<ResourceAddress>,
        nft_id: Vec<NonFungibleId>,
        /// The Lending Pool is a shared pool which shares resources with the collateral pool. This allows the lending pool to access methods from the collateral pool.
        collateral_pool: Option<ComponentAddress>,
        /// This badge creates the NFT loan. Perhaps consolidate the admin badges?
        loan_issuer_badge: Vault,
        /// The resource address of the NFT loans.
        loan_address: ResourceAddress,
        /// NFT loans are minted to create documentations of the loan that has been issued and repaid along with the loan terms.
        loans: BTreeSet<NonFungibleId>,
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
        pub fn new(user_component_address: ComponentAddress, initial_funds: Bucket, access_badge: Bucket) -> ComponentAddress {

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

            // Inserting pool info into HashMap
            let pool_resource_address = initial_funds.resource_address();
            let lending_pool: Bucket = initial_funds;
            let mut vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            let mut borrowed_vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            vaults.insert(pool_resource_address, Vault::with_bucket(lending_pool));
            borrowed_vaults.insert(pool_resource_address, Vault::new(tracking_tokens));

            // Instantiate lending pool component
            let lending_pool: ComponentAddress = Self {
                vaults: vaults,
                borrowed_vaults: borrowed_vaults,
                tracking_token_address: tracking_tokens,
                tracking_token_admin_badge: Vault::with_bucket(tracking_token_admin_badge),
                fee_vault: Vault::new(funds_resource_def),
                borrow_fees: dec!(".01"),
                interest: dec!(".02"),
                xrd_usd: dec!("1.0"),
                user_management_address: user_management_address,
                access_vault: Vault::with_bucket(access_badge),
                max_borrow: dec!("0.5"),
                min_health_factor: dec!("1.0"),
                nft_address: Vec::new(),
                nft_id: Vec::new(),
                collateral_pool: None,
                loan_issuer_badge: Vault::with_bucket(loan_issuer_badge),
                loan_address: loan_nft_address,
                loans: BTreeSet::new(),
                liquidation_component: None,
                bad_loans: BTreeSet::new(),
            }
            .instantiate().globalize();
            return lending_pool;
        }

        pub fn set_price(&mut self, user_id: NonFungibleId, xrd_price: Decimal) {
            self.xrd_usd = xrd_price;
            info!("XRD price has been set to {}", xrd_price);
            let user_management: UserManagement = self.user_management_address.into();
            let nft_resource = user_management.get_nft();
            let user_resource_manager = borrow_resource_manager!(nft_resource);
            let sbt_data: User = user_resource_manager.get_non_fungible_data(&user_id);
            let user_loans = sbt_data.loans.iter();

            for loans in user_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
                let mut loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let test = loan_data.remaining_balance;
                let col_test = loan_data.collateral_amount;
                loan_data.health_factor = ( ( col_test * xrd_price ) * dec!("0.8") ) / ( test );

                self.loan_issuer_badge.authorize(|| resource_manager.update_non_fungible_data(loans, loan_data));
            }   
        }

        /// Returns the ResourceAddress of the loan NFTs so the collateral pool component can access the NFT data.
        pub fn loan_nft(&self) -> ResourceAddress {
            return self.loan_address;
        }

        /// Sets the collateral_pool ComponentAddress so that the lending pool can access the method calls.
        pub fn set_address(&mut self, collateral_pool_address: ComponentAddress) {
            self.collateral_pool.get_or_insert(collateral_pool_address);
        }

        /// Sets the liquidation component ComponentAddress so that the lending pool can access the method calls.
        pub fn set_liquidation_address(&mut self, component_address: ComponentAddress) {
            self.liquidation_component.get_or_insert(component_address);
        }

        /// Mint tracking tokens every time there's a borrow and puts it in the borrowed vault
        fn mint_borrow(&mut self, token_address: ResourceAddress, amount: Decimal) {
            let tracking_tokens_manager: &ResourceManager = borrow_resource_manager!(self.tracking_token_address);
            let tracking_tokens: Bucket = self.tracking_token_admin_badge.authorize(|| {tracking_tokens_manager.mint(amount)});
            self.borrowed_vaults.get_mut(&token_address).unwrap().put(tracking_tokens)
        }

        /// Burn tracking tokens every time there's a repayment
        fn burn_borrow(&mut self, token_address: ResourceAddress, amount: Decimal) {
            let burn_amount: Bucket = self.borrowed_vaults.get_mut(&token_address).unwrap().take(amount);
            let tracking_tokens_manager: &ResourceManager = borrow_resource_manager!(self.tracking_token_address);
            self.tracking_token_admin_badge.authorize(|| {tracking_tokens_manager.burn(burn_amount)});
        }
        
        /// TBD
        pub fn register_user(&mut self, nft_resource_address: ResourceAddress) {
            self.nft_address.push(nft_resource_address);
        }

        /// TBD
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

            self.access_vault.authorize(|| {user_management.add_deposit_balance(user_id, token_address, dec_deposit_amount)});

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

            self.access_vault.authorize(|| {user_management.convert_collateral_to_deposit(user_id, token_address, dec_deposit_amount)});

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
        /// This method allows users to borrow funds from the pool. First, it takes the user_id to ensure that the user
        /// belongs to the pool. There are currently no checks to make sure that the user belongs to the pool because that check
        /// is done through the user_management component. It does check the borrow amount which is limited to no more than
        /// 50% of the collateral posted. In general, how the protocol detects the collateral will be through both the SBT and
        /// the loan NFT (if the user has existing loans) with the priority with the SBT. This is because the SBT can eventually be
        /// used to vouch for other users. When borrowing a simple (for now) origination fee is charged to the borrower. Transient
        /// tokens are minted so that the User Management component knows that a borrow method has been called and authorizes
        /// the change in SBT data. The tracking tokens are also minted that will be deposited to the component's borrowed vaults.
        /// This is mainly so that we can tally how much has been taken out of the pool and how much flows back in when the loans are repayed.
        /// The method then mints a loan NFT that represents the loan terms to be given to the user. The Loan NFT's NonFungibleID is registered
        /// to the SBT. Funds are withdrawn from the pool and sent to the borrower.
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
        /// * `Bucket` - Returns a bucket of the borrowed funds from the pool.
        /// * `Bucket` - Returns the loan NFT to the user.
        pub fn borrow(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, borrow_amount: Decimal) -> (Bucket, Bucket) {

            let user_management: UserManagement = self.user_management_address.into();
            let nft_resource = user_management.get_nft();

            // Check borrow percent
            let resource_manager = borrow_resource_manager!(nft_resource);
            let nft_data: User = resource_manager.get_non_fungible_data(&user_id);
            // It's unwrap because if the user does not have collateral, it will panic.
            let collateral_amount = *nft_data.collateral_balance.get(&token_address).unwrap_or(&Decimal::zero());
            assert!(borrow_amount <= collateral_amount * self.max_borrow, "Borrow amount must be less than or equals to 50% of your collateral.");

            // Calculate fees charged
            let fee = self.borrow_fees;
            let fee_charged = borrow_amount * fee;
            let actual_borrow = borrow_amount - fee_charged;

            // Calculate interest expense
            let interest_expense = borrow_amount * self.interest;

            // Minting tracking tokens to be deposited to borrowed_vault to track borrows from this pool and deposits to the pool's borrowed vault.
            self.mint_borrow(token_address, actual_borrow);

            // Mint loan NFT
            let loan_nft = self.loan_issuer_badge.authorize(|| {
                let resource_manager: &ResourceManager = borrow_resource_manager!(self.loan_address);
                resource_manager.mint_non_fungible(
                    // The User id
                    &NonFungibleId::random(),
                    // The User data
                    Loan {
                        asset: token_address,
                        collateral: token_address,
                        principal_loan_amount: borrow_amount,
                        interest: self.interest,
                        owner: user_id.clone(),
                        remaining_balance: actual_borrow + interest_expense,
                        collateral_amount: collateral_amount,
                        collateral_amount_usd: collateral_amount * self.xrd_usd,
                        health_factor: ( ( collateral_amount * self.xrd_usd ) * dec!("0.8") ) / ( actual_borrow + interest_expense ),
                        interest_expense: interest_expense,
                        loan_status: Status::Current,
                    },
                )
            });

            let loan_nft_id = loan_nft.non_fungible::<Loan>().id();

            // Commits state
            self.access_vault.authorize(|| {user_management.add_borrow_balance(user_id.clone(), token_address, actual_borrow)});
            // Insert loan NFT to the User NFT
            user_management.insert_loan(user_id.clone(), loan_nft_id.clone());

            // Inserts loan NFT to loans.
            self.loans.insert(loan_nft_id);

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let return_borrow_amount: Bucket = self.withdraw(addresses[0], actual_borrow);

            return (return_borrow_amount, loan_nft)
        }

        /// Converts the user's supply deposit to collateral.
        /// 
        /// # Description:
        /// This method converts the user's supply deposit to collateral deposit. It first checks whether the requested token to
        /// convert belongs to this pool. Takes the SBT data to view whether the user has deposits to convert to collateral.
        /// It performs another check to ensure the requested conversion is enough. The lending protocol then moves fund to the collateral
        /// component to be locked up.
        /// 
        /// This method performs a number of checks before the borrow is made:
        /// 
        /// * **Check 1:** Checks whether the resquested token to convert belongs to this lending pool.
        /// * **Check 2:** Checks whether the user has enough deposit supply to convert to collateral.
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested collateral to be converted back to supply.
        /// 
        /// * `deposit_collateral` (Decimal) - This is the amount requested to convert to collateral supply.
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned.
        pub fn convert_to_collateral(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, deposit_collateral: Decimal) {

            let pool_resource_address = self.vaults.contains_key(&token_address);
            assert!(pool_resource_address == true, "Requested asset must be the same as the lending pool.");

            let user_management: UserManagement = self.user_management_address.into();      

            // Gets the user badge ResourceAddress
            let sbt_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);

            // Check if the user has enough deposit supply to convert to collateral supply
            assert!(*sbt_data.deposit_balance.get(&token_address).unwrap() >= deposit_collateral, "Must have enough deposit supply to use as a collateral");

            let addresses: Vec<ResourceAddress> = self.addresses();
            // Creating a bucket to remove deposit supply from the lending pool to transfer to collateral pool
            let collateral_amount: Bucket = self.withdraw(addresses[0], self.vaults[&addresses[0]].amount() - deposit_collateral);
            let collateral_pool: CollateralPool = self.collateral_pool.unwrap().into();
            collateral_pool.convert_from_deposit(user_id, token_address, collateral_amount);
        }

        /// Removes the percentage of the liquidity owed to this liquidity provider.
        /// 
        /// # Description:
        /// This method is used to calculate the amount of tokens owed to the liquidity provider and take them out of
        /// the lending pool and return them to the liquidity provider.
        /// 
        /// This method performs a number of checks before liquidity removed from the pool:
        /// 
        /// * **Check 1:** 
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested amount to be redeemed.
        ///  
        /// exchange for their share of the liquidity.
        /// 
        /// * `redeem_amount` (Decimal) - This is the amount requested to redeem.
        /// 
        /// # Returns:
        /// 
        /// * `Bucket` - A Bucket of the tokens to be redeemed.
        pub fn redeem(&mut self, user_id: NonFungibleId, token_address: ResourceAddress, redeem_amount: Decimal) -> Bucket {

            // Check if the NFT belongs to this lending protocol.
            let user_management: UserManagement = self.user_management_address.into();
            let sbt_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt: User = resource_manager.get_non_fungible_data(&user_id);
            let user_loans = sbt.loans.iter();

            for loans in user_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
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
        
        /// Repays the loan in partial or in full.
        /// 
        /// # Description:
        /// This method is used to calculate the amount of tokens owed to the liquidity provider and take them out of
        /// the lending pool and return them to the liquidity provider.
        /// 
        /// This method performs a number of checks before liquidity removed from the pool:
        /// 
        /// * **Check 1:** 
        /// 
        /// # Arguments:
        /// 
        /// * `user_id` (NonFungibleId) - The NonFungibleId that identifies the specific NFT which represents the user. It is used 
        /// to update the data of the NFT.
        /// 
        /// * `token_address` (ResourceAddress) - This is the token address of the requested loan payoff.
        /// 
        /// * `repay_amount` (Decimal) - This is the amount to repay the loan.
        /// 
        /// # Returns:
        /// 
        /// * `Bucket` - A Bucket of the tokens to be redeemed.
        /// 
        /// # Design questions:
        /// * Ideally we would only need the user_id and loans are identified by the protocol as opposed to the user having to retrieve the loan NFT.
        pub fn repay(&mut self, user_id: NonFungibleId, loan_id: NonFungibleId, token_address: ResourceAddress, mut repay_amount: Bucket) -> Bucket {

            let loans = &self.loans;
            
            assert!(loans.contains(&loan_id) == true, "Requested loan repayment does not exist.");

            let user_management: UserManagement = self.user_management_address.into();
            let dec_repay_amount = repay_amount.amount();

            // Update Loan NFT
            let resource_manager = borrow_resource_manager!(self.loan_address);
            let mut loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            loan_data.remaining_balance -= dec_repay_amount;
            let interest_expense = loan_data.interest_expense;

            // Burns the tracking token for borrowed amounts
            let amount = repay_amount.amount() - interest_expense;
            self.burn_borrow(token_address, amount);

            // assert_ne!(loan_balance == Decimal::zero(), "Remaining principal amount cannot be negative.");

            if loan_data.remaining_balance == Decimal::zero() {
                loan_data.loan_status = Status::PaidOff;
                user_management.inc_paid_off(user_id.clone()); 
            } else {
                loan_data.loan_status = Status::Current;
            }

            // Commits state
            // Need to fix this
            self.loan_issuer_badge.authorize(|| resource_manager.update_non_fungible_data(&loan_id, loan_data));
            self.access_vault.authorize(|| {        });

            let to_return_amount = user_management.decrease_borrow_balance(user_id, token_address, dec_repay_amount);
            let to_return = repay_amount.take(to_return_amount);

            // Deposits the repaid loan back into the supply
            self.vaults.get_mut(&repay_amount.resource_address()).unwrap().put(repay_amount);
            to_return
        }

        /// Finds loans that are below the minimum collateral ratio allowed. 
        /// 
        /// # Description:
        /// This function essentially cycles through the loan NFTs and views the data within the NFT. As the function cycles
        /// through the NFTs, it checks the minimum collateral ratio, separating the bad loans and inserting them into 
        /// a `BTreeSet` to be queried by liquidators.
        /// 
        /// This method performs does not perform any checks.
        /// 
        /// 
        /// # Returns:
        /// 
        /// * `None` - Nothing is returned. The bad loans are inserted into the component state under `bad_loans`.
        /// 
        /// # Design questions: 
        /// Have to test whether this actually works or not
        pub fn insert_bad_loans(&mut self) {
            let loan_list = &self.loans;
            let check_loans = loan_list.iter();
            for loans in check_loans {
                let get_collateral_ratio = self.check_loan_nft(&loans);
                if get_collateral_ratio < self.min_health_factor {
                    self.bad_loans.insert(loans.clone());
                }
            };
        }

        pub fn bad_loans(&mut self) -> BTreeSet<NonFungibleId> {
            return self.bad_loans.clone();
        }

        /// Temporary method for now, might remove. Used by the liquidation component.
        pub fn get_loan_resource(&self) -> ResourceAddress {
            return self.loan_address;
        }

        /// May remove. Checks collateral ratio of the loan NFT.
        pub fn check_collateral_ratio(&self, loan_id: NonFungibleId) -> bool {
            let resource_manager = borrow_resource_manager!(self.loan_address);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            let collateral_ratio = loan_data.health_factor;
            let boolean: bool = collateral_ratio < self.min_health_factor;
            return boolean;
        }

        /// Returns a string of bad loans from bad_loans.
        pub fn get_bad_loans(&mut self) {
            // Pushes bad loans to the struct
            self.insert_bad_loans();
            let bad_loans = self.bad_loans.iter();
            for loans in bad_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
                let loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let health_factor = loan_data.health_factor;
                let loan_str = format!("Loan ID: {} health factor: {}", loans, health_factor);
                info!("{:?}", loan_str);
            };
        }

        fn check_loan_nft(&self, loan_id: &NonFungibleId) -> Decimal {
            let resource_manager = borrow_resource_manager!(self.loan_address);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            return loan_data.health_factor;
        }

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


