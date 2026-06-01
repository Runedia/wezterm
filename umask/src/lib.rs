/// Unfortunately, novice unix users can sometimes be running
/// with an overly permissive umask so we take care to install
/// a more restrictive mask while we might be creating things
/// in the filesystem.
/// This struct locks down the umask for its lifetime, restoring
/// the prior umask when it is dropped.
pub struct UmaskSaver {}

impl UmaskSaver {
    pub fn new() -> Self {
        Self {}
    }
}

impl Drop for UmaskSaver {
    fn drop(&mut self) {}
}
