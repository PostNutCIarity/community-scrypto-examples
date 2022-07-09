use scrypto::prelude::*;
use crate::user_management::*;
use crate::collateral_pool::*;
use crate::structs::{User, Loan, Status};

blueprint! {
    /// This is a struct used to define the Lending Pool.
    struct LendingPool {
        /// This is the vaults where the reserves of the tokens will be stored. The choice of storing the two
        /// vaults in a hashmap instead of storing them in two different struct variables is made to allow for an easier
        /// and more dynamic way of manipulating and making use of the vaults inside the liquidity pool. 
        vaults: HashMap<ResourceAddress, Vault>,
        /// Unsure yet how the fee design will work. One option is to deposit all the fees into a vault. Another option is 
        /// the fee will be deposited into the lending pool where there won't be much logic that may need to be implemented
        /// to claim fees. Still need to be discussed.
        supplied_amount: Decimal,
        borrow_amount: Decimal,
        fee_vault: Vault,
        /// Temporary static fee as of now. 
        borrow_fees: Decimal,
        xrd_usd: Decimal,
        user_management: ComponentAddress,
        /// Access badge to call permissioned method from the UserManagement component.
        access_vault: Vault,
        max_borrow: Decimal,
        min_health_factor: Decimal,
        min_collaterization: Decimal,
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
            let initial_funds_amount = initial_funds.amount(); 
            let lending_pool: Bucket = initial_funds;
            let mut vaults: HashMap<ResourceAddress, Vault> = HashMap::new();
            vaults.insert(pool_resource_address, Vault::with_bucket(lending_pool));

            // Instantiate lending pool component
            let lending_pool: ComponentAddress = Self {
                vaults: vaults,
                supplied_amount: initial_funds_amount,
                borrow_amount: Decimal::zero(),
                fee_vault: Vault::new(funds_resource_def),
                borrow_fees: dec!(".01"),
                xrd_usd: dec!("1.0"),
                user_management: user_management_address,
                access_vault: Vault::with_bucket(access_badge),
                max_borrow: dec!("0.5"),
                min_health_factor: dec!("1.0"),
                min_collaterization: dec!("1.5"),
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

        pub fn set_price(&mut self, user_id: NonFungibleId, _token_address: ResourceAddress, xrd_price: Decimal) {
            self.xrd_usd = xrd_price;
            info!("XRD price has been set to {}", xrd_price);
            let user_management: UserManagement = self.user_management.into();
            let nft_resource = user_management.get_nft();
            let user_resource_manager = borrow_resource_manager!(nft_resource);
            let sbt_data: User = user_resource_manager.get_non_fungible_data(&user_id);
            let user_loans = sbt_data.open_loans.iter();

            for (_token_address, loans) in user_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
                let mut loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let test = loan_data.remaining_balance;
                let col_test = loan_data.collateral_amount;
                loan_data.health_factor = ( ( col_test * xrd_price ) * dec!("0.8") ) / ( test );

                self.loan_issuer_badge.authorize(|| resource_manager.update_non_fungible_data(loans, loan_data));
            }   
        }

        pub fn retrieve_xrd_price(&self) -> Decimal {
            return self.xrd_usd;
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
            
            // Asserts that the bucket resource and the token resource address is the same.
            assert_eq!(token_address, deposit_amount.resource_address(), "Tokens must be the same.");

            // Asserts that the bucket is not empty.
            assert!(
                !deposit_amount.is_empty(), 
                "[Pool Creation]: Can't deposit an empty bucket."
            ); 
            
            let user_management: UserManagement = self.user_management.into();

            // Takes the amount passed through in the bucket.
            let dec_deposit_amount = deposit_amount.amount();

            let credit_score = 5;

            // Authorizes to increase the deposit balance of the SBT user.
            self.access_vault.authorize(|| {
                    user_management.add_deposit_balance(user_id.clone(), token_address, dec_deposit_amount)
                }
            );

            self.access_vault.authorize(|| {
                user_management.inc_credit_score(user_id, credit_score)
                }
            );

            info!("[Lending Pool]: Credit Score increased by: {:?}", credit_score);

            // Adding to supplied amount
            self.supplied_amount += deposit_amount.amount();

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
            
            let user_management: UserManagement = self.user_management.into();

            let dec_deposit_amount = deposit_amount.amount();

            self.access_vault.authorize(|| {
                user_management.convert_collateral_to_deposit(user_id, token_address, dec_deposit_amount)
                }
            );

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

            let user_management: UserManagement = self.user_management.into();
            let nft_resource = user_management.get_nft();

            // Check borrow percent
            let resource_manager = borrow_resource_manager!(nft_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            // It's unwrap because if the user does not have collateral, it will panic.
            let collateral_amount = *sbt_data.collateral_balance.get(&token_address).unwrap_or(&Decimal::zero());
            assert!(borrow_amount <= collateral_amount * self.max_borrow, "Borrow amount must be less than or equals to 50% of your collateral.");

            // Checks open loan positions
            assert_ne!(sbt_data.open_loans.contains_key(&token_address), true, "Existing loan position for {:?} already exist", token_address);

            // Calculate fees charged
            let fee = self.borrow_fees;
            let fee_charged = borrow_amount * fee;
            let actual_borrow = borrow_amount - fee_charged;

            // Minting tracking tokens to be deposited to borrowed_vault to track borrows from this pool and deposits to the pool's borrowed vault.
            self.borrow_amount += actual_borrow;

            let interest_rate = self.interest_calc(token_address);

            let modifier = user_management.credit_score_modifier(user_id.clone());

            let modified_interest_rate = interest_rate - modifier;

            // Calculate interest expense
            let interest_expense = borrow_amount * modified_interest_rate;

            let remaining_amount = actual_borrow + interest_expense;

            let health_factor = ( ( collateral_amount * self.xrd_usd ) * dec!("0.8") ) / ( actual_borrow + interest_expense );

            let liquidation_price = ( actual_borrow * self.xrd_usd ) * self.min_collaterization  / collateral_amount; 

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
                        interest: modified_interest_rate,
                        owner: user_id.clone(),
                        remaining_balance: remaining_amount,
                        collateral_amount: collateral_amount,
                        collateral_amount_usd: collateral_amount * self.xrd_usd,
                        health_factor: health_factor,
                        liquidation_price: liquidation_price,
                        interest_expense: interest_expense,
                        loan_status: Status::Current,
                    },
                )
            });

            let loan_nft_id = loan_nft.non_fungible::<Loan>().id();

            info!("[Lending Pool]: Loan NFT created.");
            info!("[Lending Pool]: Origination fee: {:?}", fee);
            info!("[Lending Pool]: Origination fee charged: {:?}", fee_charged);
            info!("[Loan NFT]: Asset: {:?}", token_address);
            info!("[Loan NFT]: Collateral: {:?}", token_address);
            info!("[Loan NFT]: Principal Loan Amount: {:?}", borrow_amount);
            info!("[Loan NFT]: Interest Rate: {:?}", modified_interest_rate);
            info!("[Loan NFT]: Owner: {:?}", user_id.clone());
            info!("[Loan NFT]: Remaining Balance: {:?}", actual_borrow);
            info!("[Loan NFT]: Collateral amount: {:?}", collateral_amount);
            info!("[Loan NFT]: Health Factor: {:?}", health_factor);
            info!("[Loan NFT]: Interest Expense: {:?}", interest_expense);

            // Commits state
            self.access_vault.authorize(|| {
                user_management.increase_borrow_balance(user_id.clone(), token_address, actual_borrow)
                }
            );
            // Insert loan NFT to the User NFT
            self.access_vault.authorize(|| {
                user_management.insert_loan(user_id.clone(), token_address, loan_nft_id.clone())
                }
            );

            // Inserts loan NFT to loans.
            self.loans.insert(loan_nft_id);

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let return_borrow_amount: Bucket = self.withdraw(addresses[0], actual_borrow);

            info!("You were able to reduce your interest rate by {:?} percent due to your credit!", modifier);
            info!(
                "Your original interest rate was {:?} and the utilization rate is {:?}",
                self.interest_calc(token_address),
                self.check_utilization_rate(token_address)
            );

            return (return_borrow_amount, loan_nft)
        }

        pub fn borrow_additional(&mut self, user_id: NonFungibleId, loan_id: NonFungibleId, token_address: ResourceAddress, borrow_amount: Decimal) -> Bucket {

            let user_management: UserManagement = self.user_management.into();
            let sbt_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);

            // Check borrow percent
            // It's unwrap because if the user does not have collateral, it will panic.
            let collateral_amount = *sbt_data.collateral_balance.get(&token_address).unwrap_or(&Decimal::zero());
            assert!(borrow_amount <= collateral_amount * self.max_borrow, "Borrow amount must be less than or equals to 50% of your collateral.");

            // Checks for open loan positions of this asset
            assert_eq!(sbt_data.open_loans.contains_key(&token_address), true, "Must have an open loan position of {:?}", token_address);

            // Calculate fees charged
            let fee = self.borrow_fees;
            let fee_charged = borrow_amount * fee;
            let actual_borrow = borrow_amount - fee_charged;

            // Calculate interest expense
            let interest_expense = borrow_amount * dec!("0.02");

            // Minting tracking tokens to be deposited to borrowed_vault to track borrows from this pool and deposits to the pool's borrowed vault.
            self.borrow_amount += actual_borrow;

            let interest_rate = self.interest_calc(token_address);

            // Change loan NFT data
            // Get the resource manager
            let mut loan_data = self.call_resource_mananger(&loan_id);
            // Asserts that loan status must be current.
            assert_eq!(loan_data.loan_status, Status::Current, "Loan status must be current.");
            // Increase borrow balance on the loan NFT.
            loan_data.remaining_balance += borrow_amount + interest_expense;
            //
            loan_data.interest_expense += interest_expense;
            // Checks whether if the health factor of the loan is greater than one.
            assert!(loan_data.health_factor >= Decimal::one(), "Loan factor must be greater than one.");

            // Authorize to increase borrow balance of the user
            self.access_vault.authorize(|| {
                user_management.increase_borrow_balance(user_id, token_address, borrow_amount)
                }
            );

            // Authorize to increase borrow balance of the loan NFT
            self.authorize_update(&loan_id, loan_data);

            // Withdrawing the amount of tokens owed to this lender
            let addresses: Vec<ResourceAddress> = self.addresses();
            let bucket: Bucket = self.withdraw(addresses[0], borrow_amount);
            return bucket;
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

            let user_management: UserManagement = self.user_management.into();      

            // Gets the user badge ResourceAddress
            let sbt_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);

            // Check if the user has enough deposit supply to convert to collateral supply
            assert!(*sbt_data.deposit_balance.get(&token_address).unwrap() >= deposit_collateral, "Must have enough deposit supply to use as a collateral");

            let addresses: Vec<ResourceAddress> = self.addresses();
            // Creating a bucket to remove deposit supply from the lending pool to transfer to collateral pool
            let collateral_amount: Bucket = self.withdraw(addresses[0], deposit_collateral);
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
            let user_management: UserManagement = self.user_management.into();
            let sbt_resource = user_management.get_nft();
            let resource_manager = borrow_resource_manager!(sbt_resource);
            let sbt_data: User = resource_manager.get_non_fungible_data(&user_id);
            let user_loans = sbt_data.open_loans.iter();

            for (_token_address, loans) in user_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
                let loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let check_paid_off = loan_data.loan_status;
                assert!(check_paid_off != Status::Current, "Must pay off loans before redeeming.");
            }
            
            // Reduce deposit balance of the user
            self.access_vault.authorize(|| {
                user_management.decrease_deposit_balance(user_id, token_address, redeem_amount)
                }
            );

            // Calculate & of the pool to be removed
            let vault: &mut Vault = self.vaults.get_mut(&token_address).unwrap();
            let redeem_amount = redeem_amount * ( vault.amount() / self.supplied_amount );
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

            let user_management: UserManagement = self.user_management.into();
            let dec_repay_amount = repay_amount.amount();

            // Update Loan NFT
            // Borrow resource manager
            let mut loan_data = self.call_resource_mananger(&loan_id);

            assert_ne!(loan_data.loan_status, Status::PaidOff, "The loan has already been paid off!");

            // Update remaining balance (includes interest expense)
            loan_data.remaining_balance -= dec_repay_amount;

            // Takes interest expense amount
            let interest_expense = loan_data.interest_expense;

            // Takes separates interest expense from principal loan amount
            let actual_repay_amount = repay_amount.amount() - interest_expense;

            // Decrease borrow counter (excludes interest expense)
            self.borrow_amount -= actual_repay_amount;

            let credit_score = 20;

            if loan_data.remaining_balance <= Decimal::zero() {
                // Change loan status to paid off
                loan_data.loan_status = Status::PaidOff;
                loan_data.remaining_balance = Decimal::zero();
                info!("[Lending Pool]: Your loan has been paid off!");

                self.access_vault.authorize(|| {
                    user_management.inc_credit_score(user_id.clone(), credit_score)
                    }
                );

                // Authorize SBT data change
                self.access_vault.authorize(|| {
                    user_management.inc_credit_score(user_id.clone(), credit_score)
                    }
                );

                info!("[Lending Pool]: Credit Score increased by: {:?}", credit_score);

                // Authorize SBT data change
                self.access_vault.authorize(|| {
                    user_management.inc_paid_off(user_id.clone()) 
                    }
                );
                // Authorize SBT data change
                self.access_vault.authorize(|| {
                    user_management.close_loan(user_id.clone(), token_address, loan_id.clone())
                    }
                );
            } else {
                loan_data.loan_status = Status::Current;
            }

            // Commits state
            self.authorize_update(&loan_id, loan_data);

            let to_return_amount = self.access_vault.authorize(|| {
                user_management.decrease_borrow_balance(user_id.clone(), token_address, dec_repay_amount)
                }
            );

            let to_return = repay_amount.take(to_return_amount);

            // Deposits the repaid loan back into the supply
            self.vaults.get_mut(&repay_amount.resource_address()).unwrap().put(repay_amount);
            to_return
        }

        pub fn liquidate(&mut self, loan_id: NonFungibleId, token_address: ResourceAddress, repay_amount: Bucket) -> Bucket {
            
            // Check to  make sure that the loan can be liquidated
            
            assert!(self.bad_loans().contains(&loan_id) == true, "This loan cannot be liquidated.");

            // Retrieve resource manager
            let mut loan_data = self.call_resource_mananger(&loan_id);

            // Calculate amount returned
            assert!(repay_amount.amount() <= loan_data.remaining_balance * dec!("0.5"), "Max repay amount exceeded.");

            // Calculate owed to liquidator (amount paid + liquidation bonus fee of 5%)
            let amount_to_liquidator = repay_amount.amount() + (repay_amount.amount() * dec!("0.05"));
            
            // Retrieve Collateral Component
            let collateral_pool: CollateralPool = self.collateral_pool.unwrap().into();

            // Take collateral owed to liquidator
            let claim_liquidation: Bucket = self.access_vault.authorize(|| 
                collateral_pool.withdraw_vault(amount_to_liquidator)
            );
            
            // Update loan
            loan_data.collateral_amount -= claim_liquidation.amount();
            //let new_collateral_amount = loan_data.collateral_amount;
            //let remaining_balance = loan_data.remaining_balance;
            //let health_factor = ( ( new_collateral_amount * self.xrd_usd ) * dec!("0.8") ) / remaining_balance;
            //loan_data.health_factor = health_factor;
            loan_data.loan_status = Status::Defaulted;

            self.authorize_update(&loan_id, loan_data);
            self.default_loan(loan_id.clone());

            // Retrieve resource manager
            let loan_data = self.call_resource_mananger(&loan_id);
            
            // Retrieve owner of the loan
            let user_id = loan_data.owner;

            // Update User State to record default amount
            let user_management: UserManagement = self.user_management.into();
            self.access_vault.authorize(|| 
                user_management.inc_default(user_id.clone())
            );

            let credit_score_decrease = 80;
            // Update User State to decrease credit score
            self.access_vault.authorize(|| 
                user_management.dec_credit_score(user_id.clone(), credit_score_decrease)
            );

            // Update user collateral balance
            self.access_vault.authorize(|| 
                user_management.decrease_collateral_balance(user_id.clone(), token_address, claim_liquidation.amount())
            );

            // Sends the repay amount to the lending pool
            self.vaults.get_mut(&repay_amount.resource_address()).unwrap().put(repay_amount);

            return claim_liquidation
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
                let health_factor = self.check_health_factor(&loans);
                if health_factor < self.min_health_factor {
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
        pub fn find_bad_loans(&mut self) {
            // Pushes bad loans to the struct
            self.insert_bad_loans();
            self.remove_closed_loans();
            let bad_loans = self.bad_loans.iter();
            for loans in bad_loans {
                let resource_manager = borrow_resource_manager!(self.loan_address);
                let loan_data: Loan = resource_manager.get_non_fungible_data(&loans);
                let health_factor = loan_data.health_factor;
                let loan_str = format!("Loan ID: {}, Health Factor: {}", loans, health_factor);
                info!("{:?}", loan_str);
            };
        }
                        
        pub fn clean_bad_loans(&self) -> Vec<NonFungibleId>
        {
            let bad_loans = self.bad_loans.iter();
            let mut paid_loan: Vec<NonFungibleId> = Vec::new();
            for loans in bad_loans {
                let health_factor = self.check_health_factor(&loans);
                let loan_data = self.call_resource_mananger(&loans);
                let loan_status = loan_data.loan_status;
                if health_factor >= self.min_health_factor {
                    paid_loan.push(loans.clone())
                } else if loan_status == Status::PaidOff {
                    paid_loan.push(loans.clone())
                }
            };

            paid_loan
        }

        fn remove_closed_loans(&mut self) 
        {
            let loans: Vec<NonFungibleId> = self.clean_bad_loans();
            if loans.is_empty() {
                {}
            } else {
                self.bad_loans.remove(&loans[0]);
            }
        }

        pub fn default_loan(&mut self, loan_id: NonFungibleId) {
            let mut loan_data = self.call_resource_mananger(&loan_id);
            loan_data.loan_status = Status::Defaulted;
            self.authorize_update(&loan_id, loan_data)
        }
        
        fn call_resource_mananger(&self, loan_id: &NonFungibleId) -> Loan {
            let resource_manager = borrow_resource_manager!(self.loan_address);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            return loan_data
        }

        fn authorize_update(&mut self, loan_id: &NonFungibleId, loan_data: Loan) {
            let resource_manager = borrow_resource_manager!(self.loan_address);
            self.loan_issuer_badge.authorize(|| resource_manager.update_non_fungible_data(&loan_id, loan_data));
        }

        pub fn interest_calc(&mut self, token_address: ResourceAddress) -> Decimal 
        {
            let utilization_rate = self.check_utilization_rate(token_address);

            let interest = if utilization_rate > dec!(".90") {
                dec!("0.12")
            } else if utilization_rate > dec!(".80") && utilization_rate <= dec!(".90") {
                dec!("0.11")
            } else if utilization_rate > dec!(".70") && utilization_rate <= dec!(".80") {
                dec!("0.10")
            } else if utilization_rate > dec!(".60") && utilization_rate <= dec!(".70") {
                dec!("0.09")
            } else {
                dec!("0.08")
            };

            return interest
        }

        fn check_health_factor(&self, loan_id: &NonFungibleId) -> Decimal 
        {
            let resource_manager = borrow_resource_manager!(self.loan_address);
            let loan_data: Loan = resource_manager.get_non_fungible_data(&loan_id);
            return loan_data.health_factor;
        }

        pub fn check_liquidity(&mut self, token_address: ResourceAddress) -> Decimal
        {
            let vault: &mut Vault = self.vaults.get_mut(&token_address).unwrap();
            info!("The liquidity of this pool is {:?}", vault.amount());
            return vault.amount()
        }

        pub fn check_utilization_rate(&mut self, token_address: ResourceAddress) -> Decimal
        {
            let borrow_amount = self.borrow_amount;
            let liquidity_amount: Decimal = borrow_amount / self.supplied_amount;
            info!("The utilization rate of this pool is {:?}", liquidity_amount);
            return liquidity_amount
        }

        pub fn check_total_supplied(&self, token_address: ResourceAddress) -> Decimal
        {
            info!("The total supplied in this pool is {:?}", self.supplied_amount);
            return self.supplied_amount
        }
        
        pub fn check_total_borrowed(&self) -> Decimal
        {
            let borrow_amount = self.borrow_amount;
            info!("The total amount borrowed from this pool is {:?}", borrow_amount);
            return borrow_amount
        }

        pub fn get_loan_info(&self, loan_id: NonFungibleId)
        {
            let loan_data = self.call_resource_mananger(&loan_id);
            let asset = loan_data.asset;
            let collateral = loan_data.collateral;
            let principal_loan_amount = loan_data.principal_loan_amount;
            let interest_rate = loan_data.interest;
            let owner = loan_data.owner;
            let remaining_balance = loan_data.remaining_balance;
            let interest_expense = loan_data.interest_expense;
            let collateral_amount = loan_data.collateral_amount;
            let collateral_amount_usd = loan_data.collateral_amount_usd;
            let health_factor = loan_data.health_factor;
            let loan_status = loan_data.loan_status;

            info!("[Loan NFT]: Loan ID: {:?}", loan_id);
            info!("[Loan NFT]: Asset: {:?}", asset);
            info!("[Loan NFT]: Collateral: {:?}", collateral);
            info!("[Loan NFT]: Principal Loan Amount: {:?}", principal_loan_amount);
            info!("[Loan NFT]: Interest Rate: {:?}", interest_rate);
            info!("[Loan NFT]: Loan Owner: {:?}", owner);
            info!("[Loan NFT]: Remaining Balance: {:?}", remaining_balance);
            info!("[Loan NFT]: Interest Expense: {:?}", interest_expense);
            info!("[Loan NFT]: Collateral Amount: {:?}", collateral_amount);
            info!("[Loan NFT]: Collateral Amount (USD): {:?}", collateral_amount_usd);
            info!("[Loan NFT]: Health Factor: {:?}", health_factor);
            info!("[Loan NFT]: Loan Status: {:?}", loan_status);
        }
        
    }
}


