pub trait Evaluable {
    type Context;
    type Error;

    fn evaluate(&self, context: &Self::Context) -> Result<bool, Self::Error>;
}
