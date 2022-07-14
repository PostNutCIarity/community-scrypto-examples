# Abstract
DegenFi is a lending protocol built with Scrypto. 

# Motivations
The motivation of this project started with an introduction to one of the developers I met at [Fodl](https://fodl.finance/) and explained to me the concept of folded leverage.
Due to my interest in Scrypto and the Radix Engine to which I've [written a few articles about](https://publish.obsidian.md/jake-mai/The+Biggest+Innovation+in+Crypto+Since+Smart+Contracts), I was curious to know how much easier it would be to design a similar protocol in an asset-oriented approach. Being that I didn't have any developer experience, I was hoping someone would build a prototype of this and study the process, but when I didn't find anyone, I decided to take it in my own hands. I spent a couple months learning Rust (and Scrypto a few months after) as a personal project during the start of the pandemic, but have yet to touch it for over a year. Although some of the syntax was still familiar to me when I began this project. I was surprised as to how intuitive it was to start developing this mechanic in Scrypto. While this project took a couple months to build, much of the hurdle was me learning Rust & Scrypto along the way and design questions I faced when architecting the system. Should depositors receive LP tokens? Should it be fungible or non-fungible? Should the liquidity supply and collateral be in the same pool or in a shared pool? What access controls should be implemented to prevent any mishaps? My perspective was attempting to solve it from a prospective user's perspective. Is it easy to use the features? How much do they have to think? What is the experience like? Will it be economical for the user? While some of the design questions have still been left unanswered, I feel like I've built a pretty interesting prototype to continue tinkering and exploring the mechanics and design. Suffice to say, for someone like me who couldn't get past the "Hello World" chapter when I attempted to learn C++ a few years ago to be able to build something remotely close to a real lending protocol, I think is a testament to how powerful Scrypto and the Radix Engine is. 

# Basic Features:

* **Multi collateral support** - Allows for the creation of multiple pools and borrow against multiple collaterals.
* **Create user** - Allows people to create users to use the protocol. An SBT will be received which will track interactions of the protocol.
* **Deposit** - Allows users to deposit liquidity into the lending pool and earn protocol fees.
* **Add collateral** - Allows users to deposit collateral into the pool to be (currently) locked away and used to overcollaterize their loan(s).
* **Add additional collateral** - Allows users to top off on collateral towards their open loan position.
* **Borrow** - Allows users to borrow from the liquidity pool with a (currently) static max borrow of 75%.
* **Borrow additional** - Allows users to top off on their open loan position.
* **Repay** - Allows users to repay their loan in partial or in full.
* **Convert deposit to collateral** - Allows user to convert their deposits to be collateralized for their loans. User (currently) do not earn protocol fees for any collateral
deposited.
* **Convert collateral to deposit** - Allows user to convert their unused collateral to be used as supply liquidity to earn protocol fees.
* **Liquidate** - Allows users to liquidate any loans that have a Health Factor of 1 or below.
* **Find bad loans** - Allows users to query loans that have below a Health Factor of 1 or below.
* **Check liquidity** - Allows users to view the liquidity available to borrow or withdraw deposits from the protocol.
* **Check total supply** - Allows users to check the total that's been supplied to the pool.
* **Check total borrowed** - Allows users to check the total that's been borrowed from the pool.
* **Check utilization rate** - Allows users to check the rate that has been borrowed against the total supply of the pool.
* **Check total collaterization supply** - Allows user to check the total collaterization that's been supplied in the pool.
* **Check SBT information** - Allows users to review their own SBT data info. 
* **Check loan information** - Allows users to view loan information of the given loan ID. 

The new transaction model introduced with v0.3.0 of Scrypto allows for the creation of composable transactions; this means that a concept such flash loans no longer needs to be implemented in the smart contract itself and that it can instead be an assertion in the transaction manifest file that performs the flash loan. In the case of DegenFi, flash loan compatible methods are implemented on the lending pool components so that users have the choice of how they wish to use flash loans: either by using these dedicated methods in a later section or by writing their transaction manifest files for their flash loans.

# Advanced Features:

* **Folded leverage** - Folded leverage is where a user deposits collateral on a lending platform, borrows against their collateral, re-deposits what they borrowed as additional collateral, borrows against the newly added collateral etc etc until the desired leverage is achieved.
* **Flash loan liquidate** - Users can liquidate a position even if they do not have the funds to repay the loan by using flash loans.

# Misc. features:

* **Get price** - Retrieves the price of a given asset using a pseudo price oracle.
* **Set price** - Sets the price of a given asset to demonstrate how liquidations work.
* **Set credit score** - Sets the desired credit score to demonstrate how the credit score system works.

# Folded Leverage

As mentioned Folded leverage is where a user deposits collateral on a lending platform, borrows against their collateral, re-deposits what they borrowed as additional collateral, borrows against the newly added collateral etc etc until the desired leverage is achieved. 

Here are the steps to open a leveraged position:

1.) You have 1,000 XRD
2.) You do a flash loan to borrow 3,000 XRD
3.) You deposit 4,000 XRD as collateral
4.) You borrow 3,000 USD (75% of XRD collateral assuming XRD is $1)
5.) You swap 3,000 USD for 3,000 XRD using Radiswap
6.) You pay back your 3,000 XRD flash loan you took out in step 2.

Users do this to earn a multiple of COMP tokens than they would have if they used the protocol normally. I've immitated this mechanic by creating a supply of protocol
tokens called "Degen Tokens" with a similar mechanic of how COMP tokens are rewarded to users by interacting with the protocol.

To close your position
1.) You take out a flash loan to cover the USD your entire loan balance. 
2.) You repay all your 3,000 USD loan balance (+ plus fees).
3.) You redeem your 4,000 XRD collateral.
4.) You swap your XRD to USD just enough to repay the flash loan in step 1.
5.) You've now exited your position.

# Flash Liquidation

In the event that there is a loan with a Health Factor below 1 that you may wish to liquidate but do not have the funds to liquidate the position. You may use flash loans to compose
together a series of transaction in which repays the loan the loan, receive that value + liquidation fee, and repay the flash loan within one transaction.

To perform a flash loan liquidation:
1.) You do a flash loan to borrow the amount you wish to repay back the loan.
2.) You liquidate the loan....
2.) You receive the collateral value + liquidation fee.
3.) You swap enough of the collateral asset to the asset you repaid the loan with.
4.) You pay back the flash loan you took in step 1.

# Design details

There are a few notable Non Fungible Tokens ("Non Fungible Token) and Soul Bounded Tokens ("SBT") that are implemented in this prototype. I've went back and forth to how
loans and users should be represented in this lending protocol and even still, I don't think I've arrived at a final conclusion yet. Albeit, this is current approach to how I envisioned it in my head.

**User/Credit Report/SBT**
User NFT is an NFT that represents users for this protocol. This NFT contains all the records of the user interacting with this protocol. It can be seen as a credit report for the user. It is also used for authorization that this user belongs to the protocol and access protocol features. Users themselves do not have permission to change the data contained within the NFT. It is a non-transferable token or otherwise known as a "Soul Bound Token" or "SBT" for short. I've implemented HashMaps to contain deposit, collateral, and borrow balance is for better flexibility and user experience. I'd like to have the user be able to view all their loans, deposit, borrow balance, etc. easily. Also, especially when it comes to repaying loans. When a loan is paid off, users do not have to worry about sending the wrong NFT, the protocol will simply look at the SBT token and find the loan that the user wants to pay off. The design of the credit score hasn't been fully thought out, but more so for demonstration purposes as to how easy it is to have this type of capability on Radix. Currently, to have one metric to underwrite creditworthiness is see how many times a borrower has paid off their loans as this will provide a track record of their borrowing history. The borrower will receive a 5 credit score increment starting from 20, 25, 30, 35, 40 everytime they pay off their loan and have a remaining paid off balance of 75% or below, 50% or below, 25% or below, and 0%. Certainly, there are ways for people to game this system by simply only taking $100 worth of loans for example and paying it off in 25% increments to get a full credit score of 150 received. As of now, it is only a demonstration of features rather than engineering a credit system.

The credit system is primitive at this point. Users who have 100, 200, and 300 credit score will receive a discount of 1%, 2%, and 3% on their interest rate respectively. Likewise, users who have 100, 200, or 300 credit score will also be allowed to decrease their collaterization requirement by 5%, 10%, and 15% respectively. 

**Loan NFT**
This is an NFT that represents the loan terms. We can consider this NFT as loan documents and hopefully in the future can be represented as legal documents or a digital representation of a legal document. This NFT is given to the borrower. For now its purpose is to simply tract the health factor of the loan, collaterization, etc. If the loan is in bad health, liquidators can query the liquidation component to evaluate bad loans and liquidate the loan's collateral. Another purpose is to track the status of the loan to update the user's credit report. In the future, given the nature that, unlike the SBT; these loan NFTs can be transferrable to which there may be interesting use cases that we can explore to securitize the loans or package them together.

**Liquidation**

The liquidation mechanic in this protocol is a simplified imitaiton of AAVE's liquidation mechanics which can be viewed [here](https://docs.aave.com/faq/liquidations#:~:text=A%20liquidation%20is%20a%20process,in%20value%20against%20each%20other).

A liquidation is a process that occurs when a borrower's health factor goes below 1 due to their collateral value not properly covering their loan/debt value. This might happen when the collateral decreases in value or the borrowed debt increases in value against each other. This collateral vs loan value ratio is shown in the health factor.

In a liquidation, up to 50% of a borrower's debt is repaid and that value + liquidation fee is taken from the collateral available, so after a liquidation that amount liquidated from your debt is repaid.

In the even that the loan reaches a Health Factor of 0.5 or below, liquidators can now pay up to 100% of a borrower's debt and that value + liquidation fee is taken from the collateral available.

The liquidation fee or liquidation bonus is currently a static 5% attirubtion to the liquidator.


