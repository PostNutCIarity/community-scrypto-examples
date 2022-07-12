Folded leverage is where a user deposits collateral on a lending platform, borrows against their collateral, re-deposits what they borrowed as additional collateral, borrows against the newly added collateral etc etc until the desired leverage is achieved.

This would allow user to:

1. Deposit token A collateral to borrow
2. Use flash loan to borrow more token A
3. Deposit borrowed token A
4. Borrow token B with borrowed collateral of token A
5. Repeat until desired leverage
6. Swap token B for token A using a DEX and pay back the flash loan

06/07/2022 - While the barebone implementation of the feature has been tested and has worked on a basic lending protocol. I am attempting to build a bit more complex lending protocol to test the feature a bit more and explore additional use-cases.

Instructions

