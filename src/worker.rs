/// Worker.
/// 
/// A worker responsible for the update of a given state.
pub trait Worker<T> {
	fn cycle(&mut self, state: &T);

	fn apply(&mut self, state: &mut T);
}