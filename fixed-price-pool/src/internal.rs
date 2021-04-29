use crate::*;

impl Contract {

    pub fn assert_owner_calling(&self) {
        assert!(
            env::predecessor_account_id() == self.owner_id,
            "can only be called by the owner"
        );
    }

    pub fn assert_can_operate(&self) {
        assert!(self.can_operate(),"operation is not open");
    }

}
