use std::sync::{Condvar, Mutex};

/// Replace obsolete demand without accumulating an unbounded FIFO.
pub struct Latest<T> {
    state: Mutex<(Option<T>, bool)>,
    changed: Condvar,
}
impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new((None, false)),
            changed: Condvar::new(),
        }
    }
}
impl<T> Latest<T> {
    pub fn replace(&self, value: T) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.1 {
            return false;
        }
        state.0 = Some(value);
        self.changed.notify_one();
        true
    }
    pub fn take(&self) -> Option<T> {
        let mut state = self.state.lock().unwrap();
        while state.0.is_none() && !state.1 {
            state = self.changed.wait(state).unwrap();
        }
        if state.1 {
            None
        } else {
            state.0.take()
        }
    }
    pub fn try_take(&self) -> Option<T> {
        self.state.lock().unwrap().0.take()
    }
    pub fn close(&self) {
        let mut state = self.state.lock().unwrap();
        state.1 = true;
        state.0 = None;
        self.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demand_is_replaced_and_close_wakes_waiter() {
        let slot = std::sync::Arc::new(Latest::default());
        slot.replace(1);
        slot.replace(2);
        assert_eq!(slot.take(), Some(2));
        let waiting = slot.clone();
        let handle = std::thread::spawn(move || waiting.take());
        slot.close();
        assert_eq!(handle.join().unwrap(), None);
        assert!(!slot.replace(3));
    }
}
