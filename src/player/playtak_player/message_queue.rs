//
// This file is part of Takkerus.
//
// Takkerus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Takkerus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Takkerus. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016-2017 Chris Foster
//

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};

#[derive(Clone)]
pub struct MessageQueue {
    queue: Arc<Mutex<Vec<String>>>,
    sender: Sender<()>,
    receiver: Arc<Mutex<Receiver<()>>>,
    disconnected: Arc<Mutex<bool>>,
}

impl MessageQueue {
    pub fn new() -> MessageQueue {
        let (sender, receiver) = mpsc::channel();

        MessageQueue {
            queue: Arc::new(Mutex::new(Vec::new())),
            sender: sender,
            receiver: Arc::new(Mutex::new(receiver)),
            disconnected: Arc::new(Mutex::new(false)),
        }
    }

    // Remove and return the first item in the queue.
    // Block if there are none in the queue.
    pub fn iter(&self) -> Iter<impl FnMut(&String) -> bool> {
        self.iter_select(|_| true)
    }

    // Remove and return the first item that fits the predicate, leaving the others in the queue.
    // Block if there are none in the queue that fit the predicate.
    pub fn iter_select<P>(&self, predicate: P) -> Iter<P> where P: FnMut(&String) -> bool {
        Iter {
            message_queue: self,
            predicate: predicate,
        }
    }

    pub fn push(&mut self, message: String) {
        self.queue.lock().unwrap().push(message);
        self.sender.send(()).ok();
    }

    pub fn disconnect(&mut self) {
        *self.disconnected.lock().unwrap() = true;
        self.sender.send(()).ok();
    }

    fn is_disconnected(&self) -> bool {
        *self.disconnected.lock().unwrap()
    }
}

pub struct Iter<'a, P> where
    P: FnMut(&String) -> bool {
    message_queue: &'a MessageQueue,
    predicate: P,
}

impl<'a, P> Iter<'a, P> where
    P: FnMut(&String) -> bool {
    pub fn peek(&mut self) -> Option<String> {
        self.get_next(false)
    }

    fn get_next(&mut self, remove: bool) -> Option<String> {
        if self.message_queue.is_disconnected() {
            return None;
        }

        if let Ok(mut queue) = self.message_queue.queue.lock() {
            if let Some(message) = self.filter_queue(&mut queue, remove) {
                return Some(message);
            }
        } else {
            panic!("Poisoned mutex");
        }

        let receiver = self.message_queue.receiver.lock().unwrap();
        loop {
            receiver.recv().ok();

            if self.message_queue.is_disconnected() {
                return None;
            }

            let mut queue = self.message_queue.queue.lock().unwrap();

            if let Some(message) = self.filter_queue(&mut queue, remove) {
                return Some(message);
            }
        }
    }

    fn filter_queue(&mut self, queue: &mut Vec<String>, remove: bool) -> Option<String> {
        if !queue.is_empty() {
            let mut match_index = None;
            for (index, message) in queue.iter().enumerate() {
                if (self.predicate)(message) {
                    match_index = Some(index);
                    break;
                }
            }

            if let Some(index) = match_index {
                let message = if remove {
                    queue.remove(index)
                } else {
                    queue[index].clone()
                };

                return Some(message);
            }
        }

        None
    }
}

impl<'a, P> Iterator for Iter<'a, P> where
    P: FnMut(&String) -> bool {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        self.get_next(true)
    }
}
