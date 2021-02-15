## Operator Functions

There are 3 "heartbeat()" functions:


### 1. distribute_staking()

```
    /// operator method -------------------------------------------------
    /// distribute_staking(). Do staking in batches of at most 100Kn
    /// returns "true" if the operator needs to call this fn again
    pub fn distribute_staking(&mut self) -> bool 
```

This fn does staking if needed, according to staking-pool weight (% of the pool)

This fns should be called preferently before the epoch ends. Leaving NEAR unstaked is benefical for the NSLP clearing
so this function should not be called when not necessary.

Once called, if distribute_staking() returns "true", the operator should call it again until it returns "false"


###  2. distribute_unstaking()

```
    // Operator method, but open to anyone
    /// distribute_unstaking(). Do unstaking 
    /// returns "true" if needs to be called again
    pub fn distribute_unstaking(&mut self) -> bool 
```

This fn does unstaking if needed, according to staking-pool weight (% of the pool)

This should be called only at the begining of each epoch, to maximize rewards received from the pools (from the previous epoch).

Once called, if the fn returns "true", the operator should call it again until it returns "false"

###  3. distribute_rewards()
```
    //------------------------------------------------------------------------
    //-- COMPUTE AND DISTRIBUTE STAKING REWARDS for a specific staking-pool --
    //------------------------------------------------------------------------
    // Operator method, but open to anyone. Should be called once per epoch per sp, after sp rewards distribution (ping)
    /// Retrieves total balance from the staking pool and remembers it internally.
    /// Also computes and distributes rewards operator and delegators
    /// this fn queries the staking pool (makes a cross-contract call)
    pub fn distribute_rewards(&mut self, sp_inx_i32: i32) -> void 
```


###  4. pub fn withdraw_from_a_pool(&mut self, inx:u16)

```
    // Operator method, but open to anyone
    //----------------------------------------------------------------------
    //  WITHDRAW FROM ONE OF THE POOLS ONCE THE WAITING PERIOD HAS ELAPSED
    //----------------------------------------------------------------------
    /// launchs a withdrawal call
    /// returns the amount withdrawn
    pub fn retrieve_funds_from_a_pool(&mut self, inx:u16) -> Promise -> u128 {

        //Note: In order to make fund-recovering independent from the operator
        //this fn is open to be called by anyone
```

This fn performs withdraw from a specific pool, in order to have the funds available when the user requests them

This should be called at the begining of each epoch. The operator should call `get_staking_pool_list()`
and process the list calling `retrieve_funds_from_a_pool` for each pool needing that

