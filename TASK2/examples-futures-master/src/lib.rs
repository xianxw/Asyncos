use std::{
	cmp::Ordering,
	collections::{BinaryHeap, HashMap},
	future::Future,
	mem,
	pin::Pin,
	sync::{
		mpsc::{channel, Sender},
		Arc, Condvar, Mutex, Weak,
	},
	task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
	thread::{self, JoinHandle},
	time::{Duration, Instant},
};

const YIELD_DELAY_SECONDS: u64 = 1;
const AGING_PRIORITY_GAP: u8 = 1;
const STARVATION_THRESHOLD_SECONDS: u64 = 2;

#[derive(Default)]
struct Parker(Mutex<bool>, Condvar);

impl Parker {
	fn park(&self) {
		let mut resumable = self.0.lock().unwrap();
		while !*resumable {
			resumable = self.1.wait(resumable).unwrap();
		}
		*resumable = false;
	}

	fn unpark(&self) {
		*self.0.lock().unwrap() = true;
		self.1.notify_one();
	}
}

#[derive(Clone)]
pub struct Executor {
	inner: Arc<ExecutorInner>,
	tasks: Arc<Mutex<HashMap<usize, Task>>>,
}

struct ExecutorInner {
	ready_queue: Mutex<ReadyQueue>,
	parker: Parker,
}

impl Executor {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(ExecutorInner {
				ready_queue: Mutex::new(ReadyQueue::default()),
				parker: Parker::default(),
			}),
			tasks: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	pub fn spawn(&self, task: Task) {
		let task_id = task.id;
		let priority = task.priority;

		{
			let mut tasks = self.tasks.lock().unwrap();
            //重复入队
			if tasks.insert(task_id, task).is_some() {
				panic!("Tried to insert a task with id: '{}', twice!", task_id);
			}
		}

		self.inner.enqueue(task_id, priority);
	}

	pub fn run(&self, start: Instant) {
		loop {
			if let Some(ready_task) = self.pop_ready_task() {
				self.poll_ready_task(ready_task, start);
				continue;
			}

			if self.is_idle() {
				break;
			}

			self.inner.parker.park();
		}
	}

	fn pop_ready_task(&self) -> Option<ReadyTask> {
		self.inner.ready_queue.lock().unwrap().pop()
	}

	fn poll_ready_task(&self, ready_task: ReadyTask, start: Instant) {
		let mut task = {
			let mut tasks = self.tasks.lock().unwrap();
			match tasks.remove(&ready_task.task_id) {
				Some(task) => task,
				None => return,
			}
		};

		debug_assert_eq!(task.priority, ready_task.priority);

		let waker = self.task_waker(ready_task.task_id, ready_task.priority);
		let mut cx = Context::from_waker(&waker);
		let poll_result = {
			let mut pinned_task = Pin::new(&mut task);
			pinned_task.as_mut().poll(&mut cx)
		};

		match poll_result {
			Poll::Ready(val) => {
				println!("Got {} at time: {:.2}.", val, start.elapsed().as_secs_f32());
			}
			Poll::Pending => {
				self.tasks.lock().unwrap().insert(ready_task.task_id, task);
			}
		}
	}

	fn task_waker(&self, task_id: usize, priority: TaskPriority) -> Waker {
		let wake_state = Arc::new(TaskWake {
			executor: Arc::downgrade(&self.inner),
			task_id,
			priority,
		});

		task_waker_into_waker(wake_state)
	}

	fn is_idle(&self) -> bool {
		self.tasks.lock().unwrap().is_empty() && self.inner.ready_queue.lock().unwrap().is_empty()
	}
}

impl ExecutorInner {
	fn enqueue(&self, task_id: usize, priority: TaskPriority) {
		self.ready_queue
			.lock()
			.unwrap()
			.push(ReadyTask::new(task_id, priority));
		self.parker.unpark();
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
pub enum TaskPriority {
	Low = 0,
	Normal = 1,
	High = 2,
	Critical = 3,
}

impl TaskPriority {
	fn score(self) -> u8 {
		self as u8
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReadyTask {
	task_id: usize,
	priority: TaskPriority,
	enqueued_at: Instant,
}

impl ReadyTask {
	fn new(task_id: usize, priority: TaskPriority) -> Self {
		Self {
			task_id,
			priority,
			enqueued_at: Instant::now(),
		}
	}

	fn priority_gap(&self, other: &Self) -> u8 {
		self.priority.score().abs_diff(other.priority.score())
	}

	fn age_gap(&self, other: &Self) -> Duration {
		if self.enqueued_at >= other.enqueued_at {
			self.enqueued_at.duration_since(other.enqueued_at)
		} else {
			other.enqueued_at.duration_since(self.enqueued_at)
		}
	}

	fn age_override_applies(&self, other: &Self) -> bool {
		self.priority_gap(other) <= AGING_PRIORITY_GAP
			&& self.age_gap(other) >= Duration::from_secs(STARVATION_THRESHOLD_SECONDS)
	}
}

impl Ord for ReadyTask {
	fn cmp(&self, other: &Self) -> Ordering {
		if self.age_override_applies(other) {
			other
				.enqueued_at
				.cmp(&self.enqueued_at)
				.then_with(|| self.priority.cmp(&other.priority))
				.then_with(|| other.task_id.cmp(&self.task_id))
		} else {
			self.priority
				.cmp(&other.priority)
				.then_with(|| other.enqueued_at.cmp(&self.enqueued_at))
				.then_with(|| other.task_id.cmp(&self.task_id))
		}
	}
}

impl PartialOrd for ReadyTask {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Default)]
struct ReadyQueue(BinaryHeap<ReadyTask>);

impl ReadyQueue {
	fn push(&mut self, task: ReadyTask) {
		self.0.push(task);
	}

	fn pop(&mut self) -> Option<ReadyTask> {
		self.0.pop()
	}

	fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

struct TaskWake {
	executor: Weak<ExecutorInner>,
	task_id: usize,
	priority: TaskPriority,
}

impl TaskWake {
	fn wake_task(&self) {
		if let Some(executor) = self.executor.upgrade() {
			executor.enqueue(self.task_id, self.priority);
		}
	}
}

fn task_waker_clone(data: *const ()) -> RawWaker {
	let arc = unsafe { Arc::from_raw(data as *const TaskWake) };
	let cloned = arc.clone();
	mem::forget(arc);
	RawWaker::new(Arc::into_raw(cloned) as *const (), &TASK_WAKER_VTABLE)
}

fn task_waker_wake(data: *const ()) {
	let arc = unsafe { Arc::from_raw(data as *const TaskWake) };
	arc.wake_task();
}

fn task_waker_wake_by_ref(data: *const ()) {
	let wake_state = unsafe { &*(data as *const TaskWake) };
	wake_state.wake_task();
}

fn task_waker_drop(data: *const ()) {
	unsafe {
		drop(Arc::from_raw(data as *const TaskWake));
	}
}

const TASK_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
	task_waker_clone,
	task_waker_wake,
	task_waker_wake_by_ref,
	task_waker_drop,
);
//入口函数
fn task_waker_into_waker(task_wake: Arc<TaskWake>) -> Waker {
	let raw_waker = RawWaker::new(Arc::into_raw(task_wake) as *const (), &TASK_WAKER_VTABLE);
	unsafe { Waker::from_raw(raw_waker) }
}

#[derive(Clone)]
pub struct Task {
	id: usize,
	reactor: Arc<Mutex<Reactor>>,
	#[allow(dead_code)]
	data: u64,
	priority: TaskPriority,
	yields_left: usize,
}

impl Task {
	pub fn new(
		reactor: Arc<Mutex<Reactor>>,
		data: u64,
		id: usize,
		priority: TaskPriority,
		yields_left: usize,
	) -> Self {
		Task {
			id,
			reactor,
			data,
			priority,
			yields_left,
		}
	}
}

impl Future for Task {
	type Output = usize;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let mut reactor = this.reactor.lock().unwrap();

		if reactor.is_ready(this.id) {
			if this.yields_left == 0 {
				reactor.finish(this.id);
				Poll::Ready(this.id)
			} else {
				this.yields_left -= 1;
				reactor.rearm(YIELD_DELAY_SECONDS, cx.waker().clone(), this.id);
				Poll::Pending
			}
		} else if reactor.has_task(this.id) {
			reactor.update_waker(this.id, cx.waker().clone());
			Poll::Pending
		} else if this.yields_left == 0 {
			Poll::Ready(this.id)
		} else {
			this.yields_left -= 1;
			reactor.register(YIELD_DELAY_SECONDS, cx.waker().clone(), this.id);
			Poll::Pending
		}
	}
}

enum TaskState {
	Ready,
	NotReady(Waker),
	Finished,
}

pub struct Reactor {
	dispatcher: Sender<Event>,
	handle: Option<JoinHandle<()>>,
	tasks: HashMap<usize, TaskState>,
}

#[derive(Debug)]
enum Event {
	Close,
	Timeout(u64, usize),
}

impl Reactor {
	pub fn new() -> Arc<Mutex<Reactor>> {
		let (tx, rx) = channel::<Event>();
		let reactor = Arc::new(Mutex::new(Reactor {
			dispatcher: tx,
			handle: None,
			tasks: HashMap::new(),
		}));

		let reactor_clone = Arc::downgrade(&reactor);
		let handle = thread::spawn(move || {
			let mut handles = vec![];
			for event in rx {
				let reactor = reactor_clone.clone();
				match event {
					Event::Close => break,
					Event::Timeout(duration, id) => {
						let event_handle = thread::spawn(move || {
							thread::sleep(Duration::from_secs(duration));
							let reactor = reactor.upgrade().unwrap();
							reactor.lock().map(|mut r| r.wake(id)).unwrap();
						});
						handles.push(event_handle);
					}
				}
			}
			handles.into_iter().for_each(|handle| handle.join().unwrap());
		});
		reactor.lock().map(|mut r| r.handle = Some(handle)).unwrap();
		reactor
	}

	fn is_ready(&self, id: usize) -> bool {
		matches!(self.tasks.get(&id), Some(TaskState::Ready))
	}

	fn has_task(&self, id: usize) -> bool {
		self.tasks.contains_key(&id)
	}

	fn update_waker(&mut self, id: usize, waker: Waker) {
		match self.tasks.get_mut(&id).unwrap() {
			TaskState::NotReady(existing_waker) => *existing_waker = waker,
			TaskState::Ready => unreachable!(),
			TaskState::Finished => panic!("Tried to refresh a finished task: {}", id),
		}
	}

	fn register(&mut self, duration: u64, waker: Waker, id: usize) {
		if self.tasks.insert(id, TaskState::NotReady(waker)).is_some() {
			panic!("Tried to insert a task with id: '{}', twice!", id);
		}
		self.dispatcher.send(Event::Timeout(duration, id)).unwrap();
	}

	fn rearm(&mut self, duration: u64, waker: Waker, id: usize) {
		let state = self.tasks.get_mut(&id).unwrap();
		match mem::replace(state, TaskState::NotReady(waker)) {
			TaskState::Ready => {}
			TaskState::NotReady(_) => unreachable!(),
			TaskState::Finished => panic!("Tried to re-arm a finished task: {}", id),
		}
		self.dispatcher.send(Event::Timeout(duration, id)).unwrap();
	}

	fn finish(&mut self, id: usize) {
		let state = self.tasks.get_mut(&id).unwrap();
		match mem::replace(state, TaskState::Finished) {
			TaskState::Ready => {}
			TaskState::NotReady(_) => unreachable!(),
			TaskState::Finished => panic!("Called 'finish' twice on task: {}", id),
		}
	}

	fn wake(&mut self, id: usize) {
		let state = self.tasks.get_mut(&id).unwrap();
		match mem::replace(state, TaskState::Ready) {
			TaskState::NotReady(waker) => waker.wake(),
			TaskState::Finished => panic!("Called 'wake' twice on task: {}", id),
			_ => unreachable!(),
		}
	}
}

impl Drop for Reactor {
	fn drop(&mut self) {
		self.dispatcher.send(Event::Close).unwrap();
		self.handle.take().map(|h| h.join().unwrap()).unwrap();
	}
}

pub fn run_priority_demo() {
	let start = Instant::now();
	let reactor = Reactor::new();
	let executor = Executor::new();

	println!("============================================");
	println!(">>> 开始乱序装载任务...");

	executor.spawn(Task::new(
		reactor.clone(),
		100,
		1,
		TaskPriority::Low,
		3,
	));

	executor.spawn(Task::new(
		reactor.clone(),
		400,
		4,
		TaskPriority::Critical,
		3,
	));

	executor.spawn(Task::new(
		reactor.clone(),
		200,
		2,
		TaskPriority::Normal,
		3,
	));

	executor.spawn(Task::new(
		reactor.clone(),
		300,
		3,
		TaskPriority::High,
		3,
	));

	println!(">>> 任务装载完毕。期望执行顺序: Critical(4) -> High(3) -> Normal(2) -> Low(1)");
	println!(">>> 开始启动调度器...");
	println!("============================================");

	executor.run(start);

	println!("============================================");
	println!(">>> 测试执行完毕！请人工确认输出顺序。");
}

#[cfg(test)]
mod executor_tests;
