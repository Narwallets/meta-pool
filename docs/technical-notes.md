## Operator Functions

There are 3 "heartbeat()" functions:


### 1. distribute_staking()

This fn does just staking, in batches, looking for pool balancing.

This should be called before the end of the epoch, to maximize rewards received from the pools.

The operator should call distribute_staking() as many times as necessary



###  2. distribute_unstaking()

This fn does just unstaking, in batches, looking for pool balancing

This should be called only at the begining of each epoch, to maximize rewards received from the pools (from the previous epoch).

The operator should call distribute_unstaking() as many times as necessary


###  3. pub fn withdraw_from_a_pool(&mut self, inx:u16)

This fn performs withdraw from a specific pool, in order to have the funds available when the user requests them

This should be called at the begining of each epoch. The operator should call get_staking_pools()
and process the list calling withdraw for each pool that needs that

