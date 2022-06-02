// Checks the user's total tokens and deposit balance of those tokens
pub fn check_deposit_balance(&self, user_auth: Proof) -> String {
    let user_badge_data: User = user_auth.non_fungible().data();
    return info!("The user's balance information is: {:?}", user_badge_data.deposit_balance);
}

// Insert user into record hashmap
{let user_id: NonFungibleId = user_nft.non_fungible::<User>().id();
    let user: User = user_nft.non_fungible().data();
    self.user_record.insert(user_id, user);}


PS C:\Users\renee\OneDrive - VCV Digital\Documents\GitHub\community-scrypto-examples\defi\folded_leverage> resim call-method $comp deposit "1,$proof" "$xrd" "1000,$xrd"
Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(0327c98a4e122d07c64f0dd38e0761e1f5570e35f368d3f40c38aa)))), error: NotAuthorized }
Execution Time: 62 ms
Instructions:
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "create_proof_by_amount", args: [Decimal("1"), ResourceAddress("03e53bc694c16238966eba2677219ee279d167831af1a96b5b430f")] }   
├─ PopFromAuthZone
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ CallMethod { component_address: 02b008be2b486d15a2756da371dcb4f83026d3a4f3e342b2a70248, method: "deposit", args: [Proof(512u32), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(513u32)] } 
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Proof(1024u32)
├─ Proof(512u32)
├─ Bucket(1026u32)
└─ Bucket(513u32)
Logs: 1
└─ [INFO ] [Lending Protocol Supply Tokens]: Pool for 030000000000000000000000000000000000000000000000000004 already exists. Adding supply directly.
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(0327c98a4e122d07c64f0dd38e0761e1f5570e35f368d3f40c38aa)))), error: NotAuthorized }) 

Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(037540550e6eb9e7561e23c57fa41aca5d34ecf16f53b1236f6b59)))), error: NotAuthorized }
Execution Time: 66 ms
Instructions:
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "create_proof_by_amount", args: [Decimal("1"), ResourceAddress("033a0a36eaae1505c3335a61f26927a64f8b538ee43aa8d0b3e5fb")] }   
├─ PopFromAuthZone
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ CallMethod { component_address: 02b7d8dd7f6c24837c67119ef986ccc80742704f69413c122dddc3, method: "deposit", args: [Proof(512u32), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(513u32)] } 
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Proof(1024u32)
├─ Proof(512u32)
├─ Bucket(1026u32)
└─ Bucket(513u32)
Logs: 1
└─ [INFO ] [Lending Protocol Supply Tokens]: Pool for 030000000000000000000000000000000000000000000000000004 already exists. Adding supply directly.
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(037540550e6eb9e7561e23c57fa41aca5d34ecf16f53b1236f6b59)))), error: NotAuthorized })


Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(037540550e6eb9e7561e23c57fa41aca5d34ecf16f53b1236f6b59)))), error: NotAuthorized }
Execution Time: 32 ms
Instructions:
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ CallMethod { component_address: 0286c628940eaf0e35ab5ed21410c43c56e0639506e17edd0eb2f6, method: "deposit", args: [NonFungibleId("3d64675899a6119e3ec4cd3b1f307b6a"), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(512u32)] }
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Bucket(1024u32)
└─ Bucket(512u32)
Logs: 0
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(037540550e6eb9e7561e23c57fa41aca5d34ecf16f53b1236f6b59)))), error: NotAuthorized })        
PS C:\Users\renee\OneDrive - VCV Digital\Documents\GitHub\community-scrypto-examples\defi\folded_leverage>


Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03b68d6c7e8a27f8da5df18d0559e005cec9b6c5f15e142a9eaafb)))), error: NotAuthorized }
Execution Time: 61 ms
Instructions:
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "create_proof_by_amount", args: [Decimal("1"), ResourceAddress("0391b5ba1cd42e715efbf7e055877f5f2735f3e5b8f311048566fb")] }   
├─ PopFromAuthZone
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ CallMethod { component_address: 0269f6b3e7dedf43f867ccf85d62da93de6f5e6a48824a6db45406, method: "deposit", args: [Proof(512u32), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(513u32)] } 
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Proof(1024u32)
├─ Proof(512u32)
├─ Bucket(1026u32)
└─ Bucket(513u32)
Logs: 1
└─ [INFO ] [Lending Protocol Supply Tokens]: Pool for 030000000000000000000000000000000000000000000000000004 already exists. Adding supply directly.
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03b68d6c7e8a27f8da5df18d0559e005cec9b6c5f15e142a9eaafb)))), error: NotAuthorized }) 


Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03280e246ab323cebddea5135606b048895a24ff884f2c54a1bea3)))), error: NotAuthorized }
Execution Time: 57 ms
Instructions:
├─ CallMethod { component_address: 021524ffc1801723424538d925799066600ee9b804531fbad23426, method: "push_proof", args: [] }
├─ PopFromAuthZone
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ CallMethod { component_address: 021524ffc1801723424538d925799066600ee9b804531fbad23426, method: "deposit", args: [NonFungibleId("b1eadffc2a720372ac97b4af19f7170b"), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(513u32)] }
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Proof(1024u32)
├─ Proof(512u32)
├─ Bucket(1026u32)
└─ Bucket(513u32)
Logs: 0
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03280e246ab323cebddea5135606b048895a24ff884f2c54a1bea3)))), error: NotAuthorized })  

Transaction Status: AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03280e246ab323cebddea5135606b048895a24ff884f2c54a1bea3)))), error: NotAuthorized }
Execution Time: 58 ms
Instructions:
├─ CallMethod { component_address: 021524ffc1801723424538d925799066600ee9b804531fbad23426, method: "push_proof", args: [] }
├─ CallMethod { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "withdraw_by_amount", args: [Decimal("1000"), ResourceAddress("030000000000000000000000000000000000000000000000000004")] }    
├─ TakeFromWorktopByAmount { amount: 1000, resource_address: 030000000000000000000000000000000000000000000000000004 }
├─ PopFromAuthZone
├─ CallMethod { component_address: 021524ffc1801723424538d925799066600ee9b804531fbad23426, method: "deposit", args: [NonFungibleId("b1eadffc2a720372ac97b4af19f7170b"), ResourceAddress("030000000000000000000000000000000000000000000000000004"), Bucket(512u32)] }
└─ CallMethodWithAllResources { component_address: 021025cfda90adea21506170be47c67ec169e41dbbdd063d54d409, method: "deposit_batch" }
Instruction Outputs:
├─ Proof(1024u32)
├─ Bucket(1025u32)
├─ Bucket(512u32)
└─ Proof(513u32)
Logs: 0
New Entities: 0
Error: TransactionExecutionError(AuthorizationError { function: "register_resource", authorization: Protected(ProofRule(This(Resource(03280e246ab323cebddea5135606b048895a24ff884f2c54a1bea3)))), error: NotAuthorized }) 