pub struct MyBox(pub *const u64);

unsafe impl Send for MyBox {}
unsafe impl Sync for MyBox {}


#[derive(Clone)]
pub enum Attack{
    FlushReload,
    FlushFlush,
}