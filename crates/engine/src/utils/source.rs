use foundry_compilers::artifacts::{ast::SourceLocation, StateMutability, Visibility};

/// Get the source string at the given location.
///
/// # Arguments
///
/// * `id` - The source index
/// * `source` - The source string
/// * `location` - The source location. The index in the `location` must be the same as the `id` if it is present.
pub fn source_string_at_location<'a>(
    id: u32,
    source: &'a str,
    location: &SourceLocation,
) -> &'a str {
    if let Some(index) = location.index {
        assert_eq!(index as u32, id, "Source index mismatch");
    }

    source_string_at_location_unchecked(source, location)
}

/// Get the source string at the given location.
///
/// # Arguments
///
/// * `source` - The source string
/// * `location` - The source location. The index in the `location` is not checked whether it is the same as the id of the source.
pub fn source_string_at_location_unchecked<'a>(
    source: &'a str,
    location: &SourceLocation,
) -> &'a str {
    let start = location.start.unwrap_or(0);
    let end = location.length.map(|l| start + l).unwrap_or(source.len() - 1);
    &source[start..end]
}

/// Slice the source location.
///
/// # Arguments
///
/// * `src` - The source location
/// * `start` - The start index
/// * `length` - The length of the sliced source
///
/// # Returns
///
/// The sliced source location.
///
/// # Example
///
/// ```rust
/// let src = SourceLocation { start: Some(1), length: Some(10), index: Some(0) };
/// let sliced = slice_source_location(&src, 1, 3);
/// assert_eq!(sliced.start, Some(2));
/// assert_eq!(sliced.length, Some(3));
/// assert_eq!(sliced.index, Some(0));
/// ```
pub fn slice_source_location(src: &SourceLocation, start: usize, length: usize) -> SourceLocation {
    assert!(
        src.length.map(|l| l >= start).unwrap_or(true),
        "Sliced start is greater than the original source length"
    );
    assert!(
        src.length.map(|l| l >= start + length).unwrap_or(true),
        "Sliced source length is greater than the original source length"
    );

    SourceLocation {
        start: src.start.map(|s| s + start).or(Some(start)),
        length: Some(length),
        index: src.index,
    }
}

/// Convert the visibility to a string.
///
/// # Arguments
///
/// * `visibility` - The visibility
///
/// # Returns
///
/// The string representation of the visibility.
///
/// # Example
///
/// ```rust
/// let visibility = Visibility::Public;
/// let str = visibility_to_str(visibility);
/// assert_eq!(str, "public");
/// ```
pub fn visibility_to_str(visibility: &Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Internal => "internal",
        Visibility::Private => "private",
        Visibility::External => "external",
    }
}

/// Convert the mutability to a string.
///
/// # Arguments
///
/// * `mutability` - The mutability
///
/// # Returns
///
/// The string representation of the mutability.
///
/// # Example
///
/// ```rust
/// let mutability = StateMutability::Pure;
/// let str = mutability_to_str(mutability);
/// assert_eq!(str, "pure");
/// ```
pub fn mutability_to_str(mutability: &StateMutability) -> &'static str {
    match mutability {
        StateMutability::Pure => "pure",
        StateMutability::View => "view",
        StateMutability::Payable => "payable",
        StateMutability::Nonpayable => "nonpayable",
    }
}
