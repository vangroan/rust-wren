use crate::ModuleBuilder;

pub trait WrenForeignClass {
    const NAME: &'static str;

    fn register(bindings: &mut ModuleBuilder);
}
