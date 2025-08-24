//! A wrapper around `std::Vec` to ease realtime safety.

// A `Vec` was chosen over a `VecDeque` for ease-of-use.
//
// In any case, we expect these sequences to be fairly short,
// so shifting all elements when we run out of capacity
// should be quite fast.

/// A wrapper around `std::Vec` with a fixed capacity.
///
/// This preserves the capacity on clone, making it easy
/// to ensure no unexpected allocations are made after
/// construction.
///
/// The default capacity is 16 elements.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedVec<T>(Vec<T>);

// Vec<T>::clone doesn't preserve capacity, so we
// need to derive it manually.
impl<T: Clone> Clone for FixedVec<T> {
    fn clone(&self) -> Self {
        let mut inner = self.0.clone();

        inner.reserve_exact(self.0.capacity() - self.0.len());

        Self(inner)
    }
}

impl<T> Default for FixedVec<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

impl<T> FixedVec<T> {
    /// Construct a new [`FixedVec`] with a fixed capacity.
    pub fn new(capacity: usize) -> Self {
        let mut seq = Vec::new();
        seq.reserve_exact(capacity);

        Self(seq)
    }

    /// Return a slice of the underlying sequence.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Return a mutable slice of the underlying sequence.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.0
    }

    /// Push a value, returning a popped value if the
    /// maximum capacity has been reached.
    pub fn push(&mut self, value: T) -> Option<T> {
        // a bit of a degenerate case
        if self.0.capacity() == 0 {
            return None;
        }

        if self.0.len() == self.0.capacity() {
            let popped = self.0.remove(0);
            self.0.push(value);

            Some(popped)
        } else {
            self.0.push(value);

            None
        }
    }

    /// Clear the sequence, removing all values.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Return the number of elements in the sequence.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return the capacity of the underlying vector.
    ///
    /// This cannot change after construction.
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl<T> core::ops::Deref for FixedVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> core::ops::DerefMut for FixedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
