use crossbeam_channel::{
	Sender,
	Receiver
};
use super::Worker;

pub struct Thread<S> {
	initializers: Vec<Box<dyn 'static + Send + FnOnce() -> Box<dyn Worker<S>>>>,
	wake_signal: Sender<Event<S>>,
	end_signal: Receiver<()>,
	processor_signals: Option<(Receiver<Event<S>>, Sender<()>)>,
}

impl<S> Thread<S> {
	pub fn new() -> Thread<S> {
		let (ws, wr) = crossbeam_channel::bounded(1);
		let (es, er) = crossbeam_channel::bounded(1);
		Thread {
			initializers: Vec::new(),
			wake_signal: ws,
			end_signal: er,
			processor_signals: Some((wr, es))
		}
	}

	fn is_started(&self) -> bool {
		self.processor_signals.is_none()
	}

	pub fn add<T: 'static + Worker<S>, F>(&mut self, f: F) where F: 'static + Send + FnOnce() -> T {
		if !self.is_started() {
			self.initializers.push(Box::new(|| {
				Box::new(f())
			}))
		} else {
			panic!("thread already started")
		}
	}

	/// Start the thread.
	pub fn start(&mut self) where S: 'static + Sync {
		if let Some((wr, es)) = self.processor_signals.take() {
			let mut initializers = Vec::new();
			std::mem::swap(&mut self.initializers, &mut initializers);
			std::thread::spawn(move || {
				let workers: Vec<_> = initializers.into_iter().map(|f| f()).collect();
				let mut proc = Processor {
					workers,
					wake_signal: wr,
					end_signal: es
				};

				proc.start()
			});
		} else {
			panic!("thread already started")
		}
	}

	/// Start a cycle.
	///
	/// ## Safety
	/// 
	/// The state reference must live until the cycle is finished,
	/// when the next call to [wait] returns.
	pub(crate) unsafe fn cycle(&self, state: &S) {
		self.wake_signal.send(Event::Cycle(state)).unwrap()
	}

	pub(crate) fn wait(&self) {
		self.end_signal.recv().unwrap()
	}

	/// Apply changes to the state.
	///
	/// ## Safety
	/// 
	/// The thread must by pause (between two cycles) while this method is called.
	pub(crate) unsafe fn apply(&self, state: &mut S) {
		self.wake_signal.send(Event::Apply(state)).unwrap();
		self.end_signal.recv().unwrap()
	}
}

enum Event<S> {
	Cycle(*const S),
	Apply(*mut S)
}

unsafe impl<S: Sync> Send for Event<S> {}

struct Processor<S> {
	workers: Vec<Box<dyn Worker<S>>>,
	wake_signal: Receiver<Event<S>>,
	end_signal: Sender<()>
}

impl<S> Processor<S> {
	/// Start the thread.
	pub fn start(&mut self) {
		loop {
			match self.wake_signal.recv() {
				Ok(Event::Cycle(ptr)) => {
					let state = unsafe { &*ptr }; // safety is ensured by the [Thread::cycle] and [Thread::apply].
					for worker in &mut self.workers {
						worker.cycle(state);
					}
				},
				Ok(Event::Apply(ptr)) => {
					let state = unsafe { &mut *ptr }; // safety is ensured by the [Thread::cycle] and [Thread::apply].
					for worker in &mut self.workers {
						worker.apply(state);
					}
				},
				Err(e) => panic!("channel error: {:?}", e)
			}

			self.end_signal.send(()).unwrap()
		}
	}
}
