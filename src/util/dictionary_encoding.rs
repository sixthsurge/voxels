use std::hash::Hash;

use bitvec::prelude::*;
use rustc_hash::FxHashMap;

/// An heap-allocated array compressed with dictionary encoding
/// The size of each element is the fewest number of bits needed to represent the highest index in
/// the dictionary
/// For example, if the dictionary contains two entries, each element of the vec would be stored in
/// a single bit
/// The dictionary never shrinks unless `optimize` is called
#[derive(Clone, Debug)]
pub struct DictionaryEncodedArray<T>
where
    T: Clone + Eq + Hash,
{
    buffer: BitSizedIntegerBuffer,
    dictionary: Dictionary<T>,
}

impl<T> DictionaryEncodedArray<T>
where
    T: Clone + Eq + Hash,
{
    /// Create an empty `DictionaryEncodedArray` without allocating any memory
    pub fn new(len: usize) -> Self {
        Self {
            buffer: BitSizedIntegerBuffer::new(len),
            dictionary: Dictionary::new(),
        }
    }

    /// Returns the element at the given index in the `DictionaryEncodedArray`, or an `IndexError` if
    /// the index is out of bounds.
    /// This operation is O(1)
    pub fn get(&self, index: usize) -> Result<T, IndexError> {
        if self.dictionary.entries.len() == 1 && index < self.buffer.len {
            Ok(self.dictionary.entries[0].clone())
        } else {
            self.buffer
                .get(index)
                .map(|dictionary_index| self.dictionary.entries[dictionary_index].clone())
        }
    }

    /// Returns the element at the given index in the `DictionaryEncodedArray` without performing
    /// bounds checking
    /// UB if the index is out of bounds
    /// This operation is O(1)
    pub unsafe fn get_unchecked(&self, index: usize) -> T {
        if self.dictionary.entries.len() == 1 {
            self.dictionary.entries[0].clone()
        } else {
            let dictionary_index = unsafe { self.buffer.get_unchecked(index) };
            self.dictionary.entries[dictionary_index].clone()
        }
    }

    /// Update the element at the given index, if the index is within bounds
    /// This operation is O(1) if it does not grow the dictionary to the next power of two
    /// Otherwise, it is O(n), as the buffer must be rebuilt
    pub fn set(&mut self, index: usize, value: T) -> Result<(), IndexError> {
        if index < self.buffer.len {
            Ok(unsafe { self.set_unchecked(index, value) })
        } else {
            Err(IndexError(index, self.buffer.len))
        }
    }

    /// Update the element at the given index
    /// UB if the index is out of bounds
    /// This operation is O(1) if it does not grow the dictionary to the next power of two
    /// Otherwise, it is O(n), as the buffer must be rebuilt
    pub unsafe fn set_unchecked(&mut self, index: usize, value: T) {
        // update dictionary
        let dictionary_index = self.dictionary.get_or_add_index(&value);

        if self.requires_rebuilding() {
            self.buffer
                .resize_elements(self.buffer.element_size + 1);
        }

        self.buffer
            .set_unchecked(index, dictionary_index);
    }

    /// Number of elements in the array
    pub fn len(&self) -> usize {
        self.buffer.len
    }

    /// Returns an iterator over clones of all elements in the vec
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = T> + 'a {
        self.buffer
            .iter()
            .map(|dictionary_index| self.dictionary.entries[dictionary_index].clone())
    }

    /// True if the data buffer must be rebuilt as the dictionary has grown to the next power of
    /// two
    fn requires_rebuilding(&self) -> bool {
        let element_size = ceil_log2(self.dictionary.entries.len() as u32) as usize;
        self.buffer.element_size != element_size
    }
}

impl<Container, Element> From<Container> for DictionaryEncodedArray<Element>
where
    Container: AsRef<[Element]>,
    Element: Clone + Eq + Hash,
{
    /// Created a `DictionaryEncodedArray<T>` from a `&[T]`
    /// This operation is O(len)
    fn from(container: Container) -> Self {
        let elements = container.as_ref();
        if elements.len() == 0 {
            return Self::new(0);
        }

        // build dictionary
        let dictionary = Dictionary::build(elements);

        if dictionary.entries.len() == 1 {
            return Self {
                buffer: BitSizedIntegerBuffer::new(elements.len()),
                dictionary,
            };
        }

        // calculate element size
        let len = elements.len();
        let element_size = ceil_log2(dictionary.entries.len() as u32) as usize;

        // build buffer
        let len_bits = len * element_size;
        let mut data = BitVec::with_capacity(len_bits);

        unsafe {
            data.set_len(len_bits);
        }

        for (i, chunk) in data
            .chunks_exact_mut(element_size)
            .enumerate()
        {
            chunk.store_be(dictionary.index_lookup[&elements[i]]);
        }

        Self {
            buffer: BitSizedIntegerBuffer {
                data,
                len,
                element_size,
            },
            dictionary,
        }
    }
}

#[derive(Clone, Debug)]
struct BitSizedIntegerBuffer {
    data: BitVec,
    len: usize,
    element_size: usize,
}

impl BitSizedIntegerBuffer {
    fn new(len: usize) -> Self {
        Self {
            data: BitVec::new(),
            len,
            element_size: 0,
        }
    }

    /// Resize the elements to the new element size
    fn resize_elements(&mut self, new_element_size: usize) {
        let len_bits = self.len * new_element_size;
        let new_data = if self.element_size > 0 {
            let mut result = BitVec::with_capacity(len_bits);

            unsafe {
                result.set_len(len_bits);
            }

            for (dst_chunk, src_chunk) in result.chunks_mut(new_element_size).zip(
                self.data
                    .chunks_exact(self.element_size),
            ) {
                dst_chunk.store_be(src_chunk.load_be::<usize>());
            }

            result
        } else {
            BitVec::repeat(false, len_bits)
        };

        self.data = new_data;
        self.element_size = new_element_size;
    }

    fn get(&self, index: usize) -> Result<usize, IndexError> {
        if index < self.len {
            Ok(unsafe { self.get_unchecked(index) })
        } else {
            Err(IndexError(index, self.len))
        }
    }

    /// UB if the index is out of bounds
    unsafe fn get_unchecked(&self, index: usize) -> usize {
        let first_bit = index * self.element_size;
        let last_bit = first_bit + self.element_size;

        // safe because the invariants that bits.len() == len * element_size and index < len
        // guarantee that first_bit..last_bit is within the bounds of the bit array
        unsafe {
            self.data
                .get_unchecked(first_bit..last_bit)
        }
        .load_be()
    }

    /// UB if the index is out of bounds or the element size is 0
    unsafe fn set_unchecked(&mut self, index: usize, value: usize) {
        if self.element_size == 0 {
            return;
        }

        let first_bit = index * self.element_size;
        let last_bit = first_bit + self.element_size;

        // safe because the invariants that bits.len() == len * element_size and index < len
        // guarantee that first_bit..last_bit is within the bounds of the bit array
        unsafe {
            self.data
                .get_unchecked_mut(first_bit..last_bit)
                .store_be(value)
        };
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = usize> + 'a {
        self.data
            .chunks_exact(self.element_size)
            .map(|chunk| chunk.load_be())
    }
}

#[derive(Clone, Debug)]
struct Dictionary<T>
where
    T: Clone + Eq + Hash,
{
    entries: Vec<T>,
    index_lookup: FxHashMap<T, usize>,
}

impl<T> Dictionary<T>
where
    T: Clone + Eq + Hash,
{
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            index_lookup: FxHashMap::default(),
        }
    }

    /// Create a dictionary to encode `elements` - one entry per unique item
    fn build(elements: &[T]) -> Self {
        let mut entries = Vec::new();
        let mut index_lookup = FxHashMap::default();

        for element in elements {
            if !index_lookup.contains_key(element) {
                let entry_index = entries.len();
                entries.push(element.clone());
                index_lookup.insert(element.clone(), entry_index);
            }
        }

        Self {
            entries,
            index_lookup,
        }
    }

    /// Returns the index of the given value in the dictionary, creating it if it does not exist
    pub fn get_or_add_index(&mut self, value: &T) -> usize {
        self.index_lookup
            .get(&value)
            .cloned()
            .unwrap_or_else(|| {
                let next_index = self.entries.len();
                self.entries.push(value.clone());
                self.index_lookup
                    .insert(value.clone(), next_index);
                next_index
            })
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[error("index {0} out of bounds (length is {1})")]
pub struct IndexError(usize, usize);

/// Value of `ceil(log_2(x))`
/// Panics if x == 0
fn ceil_log2(x: u32) -> u32 {
    u32::BITS - (x - 1).leading_zeros()
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use crate::DictionaryEncodedArray;

    #[test]
    fn from_array() {
        let values = [0, 1, 2, 3, 4, 5, 0, 0, 0];
        let encoded = DictionaryEncodedArray::from(&values);

        assert_eq!(encoded.len(), values.len());
        assert_eq!(encoded.dictionary.entries.len(), 6);

        for (i, value) in values.into_iter().enumerate() {
            assert_eq!(Ok(value), encoded.get(i));
        }
    }

    #[test]
    fn all_same() {
        let mut encoded = DictionaryEncodedArray::from(&[0; 100]);
        assert_eq!(100, encoded.len());
        assert_eq!(0, encoded.buffer.data.len());
        assert_eq!(1, encoded.dictionary.entries.len());
        assert_eq!(Ok(0), encoded.get(0));
        encoded.set(0, 1).unwrap();
        assert_eq!(Ok(1), encoded.get(0));
    }

    #[test]
    fn set() {
        let mut encoded = DictionaryEncodedArray::from(&[0; 100]);
        let values = [0, 1, 2, 3, 4, 5, 0, 0, 0];

        for (i, value) in values.iter().enumerate() {
            encoded.set(i, *value).unwrap();
        }

        for (i, value) in values.into_iter().enumerate() {
            assert_eq!(Ok(value), encoded.get(i));
        }
    }

    #[test]
    fn from_new() {
        let mut encoded = DictionaryEncodedArray::<i32>::new(100);
        encoded.set(0, 1).unwrap();
        encoded.set(1, 2).unwrap();
        assert_eq!(Ok(1), encoded.get(0));
        assert_eq!(Ok(2), encoded.get(1));
    }

    #[test]
    fn from_empty_slice() {
        black_box(DictionaryEncodedArray::<i32>::from(&[]));
    }
}
