use jinme::prelude::*;
use crate::script::BoxedCommand;

pub trait CommandFactory: Send + Sync {
    fn create(&self, args: Vec<PtrValue>) -> Result<BoxedCommand, String>;
}

impl CommandFactory for fn(Vec<PtrValue>) -> Result<BoxedCommand, String> {
    fn create(&self, args: Vec<PtrValue>) -> Result<BoxedCommand, String> {
        self(args)
    }
}

pub type BoxedCommandFactory = Box<dyn CommandFactory>;

pub fn closure_factory<F>(f: F) -> Box<dyn CommandFactory>
where
    F: Fn(Vec<PtrValue>) -> Result<BoxedCommand, String> + Send + Sync + 'static,
{
    struct Wrapper<F> {
        f: F,
    }

    impl<F> CommandFactory for Wrapper<F>
    where
        F: Fn(Vec<PtrValue>) -> Result<BoxedCommand, String> + Send + Sync,
    {
        fn create(&self, args: Vec<PtrValue>) -> Result<BoxedCommand, String> {
            (self.f)(args)
        }
    }

    Box::new(Wrapper { f })
}
