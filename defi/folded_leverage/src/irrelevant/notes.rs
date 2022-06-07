// Checks the user's total tokens and deposit balance of those tokens
pub fn check_deposit_balance(&self, user_auth: Proof) -> String {
    let user_badge_data: User = user_auth.non_fungible().data();
    return info!("The user's balance information is: {:?}", user_badge_data.deposit_balance);
}

// Insert user into record hashmap
{let user_id: NonFungibleId = user_nft.non_fungible::<User>().id();
    let user: User = user_nft.non_fungible().data();
    self.user_record.insert(user_id, user);}

// Check lien - 06/01/22 - Make sure this makes sense
self.check_lien(&user_id, &address);

[ERROR] Panicked at 'called `Option::unwrap()` on a `None` value', src\user_management.rs:180:77

self.auth_vault.authorize(|| {
    let some_data = self
        .data
        .take_non_fungible(&NonFungibleId::from_u64(some_id));
                                                                                        
    let new_data = some_data.non_fungible::<SomeNFT>().data().new_data();
                                                                                        
    some_data
        .non_fungible::<SomeNFT>()
        .update_data(new_data);
                                                                                        
    self.data.put(new_data);
})