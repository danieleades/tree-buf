use crate::prelude::*;

// TODO: The interaction between Default and Missing here may be dubious.
// What it will ultimately infer is that the struct exists, but that all it's
// fields should also come up missing. Where this gets really sketchy though
// is that there may be no mechanism to ensure that none of it's fields actually
// do come up missing in the event of a name collision. I think what we actually
// want is to try falling back to the owning struct default implementation instead,
// but that would require Default on too much. Having the branch type be a part
// of the lookup somehow, or have missing be able to cancel the branch to something bogus may help.
//
// Ammendment to previous. This comment is somewhat out of date, now that Missing isn't really implemented,
// and that the schema match has been moved to one place.
#[derive(Copy, Clone, Default, Debug)]
pub struct Object {
    pub num_fields: usize,
}

impl Object {
    // TODO: ParentBranch no longer contain data, so just go ahead and get rid of it as an argument.
    pub fn flush<ParentBranch: StaticBranch>(self, _branch: ParentBranch, bytes: &mut Vec<u8>) {
        PrimitiveId::Object { num_fields: self.num_fields }.write(bytes);
    }
}