/// A no-frills two-dimensional array.
#[derive(Clone)]
pub(crate) struct Array2<T> {
    pub(crate) values: Vec<T>,
    pub(crate) dim: [usize; 2],
}

impl<T> Array2<T> {
    pub(crate) fn from_vec(values: Vec<T>, minor_dim: usize) -> Self {
        debug_assert!(!values.is_empty());
        debug_assert!(minor_dim > 0);
        debug_assert_eq!(values.len() % minor_dim, 0);

        let dim = [values.len() / minor_dim, minor_dim];
        Self { values, dim }
    }

    pub(crate) fn len(&self) -> usize {
        self.dim[0]
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &[T]> {
        (0..self.len()).map(|i| &self.values[i * self.dim[1]..(i + 1) * self.dim[1]])
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut [T]> {
        let ptr = self.values.as_mut_ptr();
        (0..self.len()).map(move |i| unsafe {
            std::slice::from_raw_parts_mut(ptr.add(i * self.dim[1]), self.dim[1])
        })
    }
}
