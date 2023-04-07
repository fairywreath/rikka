use std::{
    iter::repeat,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::{drop_in_place, read},
    sync::Arc,
};

use crossbeam_channel::{Receiver, Sender, TryRecvError};

#[derive(Debug)]
pub struct Escape<T> {
    value: ManuallyDrop<T>,
    sender: Sender<T>,
}

impl<T> Escape<T> {
    pub fn escape(value: T, terminal: &Terminal<T>) -> Self {
        Escape {
            value: ManuallyDrop::new(value),
            sender: Sender::clone(&terminal.sender),
        }
    }

    pub fn unescape(escape: Self) -> T {
        unsafe {
            // Prevent `<Escape<T> as Drop>::drop` from being called.
            let mut escape = ManuallyDrop::new(escape);

            let value = read(&mut *escape.value);

            drop_in_place(&mut escape.sender);

            value
        }
    }

    pub fn share(escape: Self) -> Handle<T> {
        escape.into()
    }
}

impl<T> Deref for Escape<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &*self.value
    }
}

impl<T> DerefMut for Escape<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.value
    }
}

impl<T> Drop for Escape<T> {
    fn drop(&mut self) {
        unsafe {
            match self.sender.send(read(&mut *self.value)) {
                Ok(_) => {}
                Err(_) => {
                    log::error!("`Escape::Drop` - send failed!");
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Terminal<T> {
    receiver: Receiver<T>,
    sender: ManuallyDrop<Sender<T>>,
}

impl<T> Terminal<T> {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Terminal {
            sender: ManuallyDrop::new(sender),
            receiver,
        }
    }

    pub fn escape(&self, value: T) -> Escape<T> {
        Escape::escape(value, &self)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        repeat(()).scan(&mut self.receiver, move |receiver, ()| {
            if !receiver.is_empty() {
                receiver.recv().ok()
            } else {
                None
            }
        })
    }
}

impl<T> Drop for Terminal<T> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.sender);
            match self.receiver.try_recv() {
                Err(TryRecvError::Disconnected) => {}
                _ => {
                    log::error!("Terminal must be dropped after all `Escape`s are dropped!");
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Handle<T> {
    inner: Arc<Escape<T>>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> From<Escape<T>> for Handle<T> {
    fn from(value: Escape<T>) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &**self.inner
    }
}
