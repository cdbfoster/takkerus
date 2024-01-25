//! An unordered, growable buffer of objects that reuses empty space before reallocating.

use std::mem;

pub struct Bag<T> {
    buffer: Vec<Option<T>>,
    unused: Vec<usize>,
}

impl<T> Default for Bag<T> {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            unused: Default::default(),
        }
    }
}

impl<T> Bag<T> {
    #[allow(dead_code)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            unused: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len() - self.unused.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn unused(&self) -> usize {
        self.unused.len()
    }

    pub fn push(&mut self, value: T) -> usize {
        let index = match self.unused.pop() {
            Some(index) => index,
            None => {
                self.buffer.push(None);
                self.buffer.len() - 1
            }
        };

        self.buffer[index] = Some(value);
        index
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        match self.buffer.get_mut(index).map(mem::take) {
            Some(removed) => {
                self.unused.push(index);
                removed
            }
            None => None,
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.buffer.get(index).and_then(|value| value.as_ref())
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.buffer.get_mut(index).and_then(|value| value.as_mut())
    }

    #[allow(dead_code)]
    pub fn grab_index(&self) -> Option<usize> {
        self.buffer
            .iter()
            .enumerate()
            .filter_map(|(i, value)| value.as_ref().map(|_| i))
            .next()
    }

    pub fn find_index<P>(&self, mut predicate: P) -> Option<usize>
    where
        P: FnMut(&T) -> bool,
    {
        self.buffer
            .iter()
            .enumerate()
            .filter_map(|(i, value)| value.as_ref().map(|value| (i, value)))
            .find_map(|(i, value)| predicate(value).then_some(i))
    }

    #[allow(dead_code)]
    pub fn contains(&self, index: usize) -> bool {
        self.buffer.get(index).filter(|x| x.is_some()).is_some()
    }
}
