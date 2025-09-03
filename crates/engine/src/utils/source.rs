use foundry_compilers::artifacts::{
    ast::SourceLocation, BlockOrStatement, ForStatement, StateMutability, Statement, Visibility,
};

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

/// Find the index of the next character immediately after the source location.
///
/// # Arguments
///
/// * `src` - The source location
///
/// # Returns
///
/// The index of the next character immediately after the source location.
///
pub fn find_next_index_of_source_location(src: &SourceLocation) -> Option<usize> {
    if let Some(start) = src.start {
        if let Some(length) = src.length {
            return Some(start + length);
        }
    }
    None
}

/// Find the next semicolon after the source location.
///
/// # Arguments
///
/// * `source` - The source string
/// * `src` - The source location
///
/// # Returns
///
/// The index of the next semicolon after the source location in the source string.
///
/// # Example
///
/// ```rust
/// let source = "1;2;3;4;5";
/// let src = SourceLocation { start: Some(0), length: Some(1), index: Some(0) };
/// let next_semicolon = find_next_semicolon_after_source_location(source, &src);
/// assert_eq!(next_semicolon, Some(2));
/// ```
pub fn find_next_semicolon_after_source_location(
    source: &str,
    src: &SourceLocation,
) -> Option<usize> {
    let start = src.start.unwrap_or(0);
    let end = src.length.map(|l| start + l).unwrap_or(start);
    let substr = &source[end..];
    substr.find(";").map(|i| i + end)
}

/// Find the index of the next character immediately after the `BlockOrStatement`.
///
/// # Arguments
///
/// * `source` - The source string
/// * `block_or_statement` - The `BlockOrStatement`
///
/// # Returns
///
/// The index of the next character immediately after the `BlockOrStatement`.
pub fn find_next_index_of_block_or_statement(
    source: &str,
    block_or_statement: &BlockOrStatement,
) -> Option<usize> {
    match block_or_statement {
        BlockOrStatement::Statement(statement) => find_next_index_of_statement(source, statement),
        BlockOrStatement::Block(block) => find_next_index_of_source_location(&block.src),
    }
}

/// Find the index of the next character immediately after the `Statement`.
///
/// # Arguments
///
/// * `source` - The source string
/// * `stmt` - The `Statement`
///
/// # Returns
///
/// The index of the next character immediately after the `Statement`.
pub fn find_next_index_of_statement(source: &str, stmt: &Statement) -> Option<usize> {
    match stmt {
        Statement::Block(block) => find_next_index_of_source_location(&block.src),
        Statement::Break(break_stmt) => {
            find_next_semicolon_after_source_location(source, &break_stmt.src)
        }
        Statement::Continue(continue_stmt) => {
            find_next_semicolon_after_source_location(source, &continue_stmt.src).map(|i| i + 1)
        }
        Statement::DoWhileStatement(do_while_statement) => {
            find_next_index_of_source_location(&do_while_statement.src)
        }
        Statement::EmitStatement(emit_statement) => {
            find_next_semicolon_after_source_location(source, &emit_statement.src).map(|i| i + 1)
        }
        Statement::ExpressionStatement(expression_statement) => {
            find_next_semicolon_after_source_location(source, &expression_statement.src)
                .map(|i| i + 1)
        }
        Statement::ForStatement(for_statement) => {
            find_next_index_of_block_or_statement(source, &for_statement.body)
        }
        Statement::IfStatement(if_statement) => match &if_statement.false_body {
            Some(false_body) => find_next_index_of_block_or_statement(source, false_body),
            None => find_next_index_of_block_or_statement(source, &if_statement.true_body),
        },
        Statement::InlineAssembly(inline_assembly) => {
            find_next_index_of_source_location(&inline_assembly.src)
        }
        Statement::PlaceholderStatement(placeholder_statement) => {
            find_next_semicolon_after_source_location(source, &placeholder_statement.src)
                .map(|i| i + 1)
        }
        Statement::Return(return_stmt) => find_next_index_of_source_location(&return_stmt.src),
        Statement::RevertStatement(revert_statement) => {
            find_next_semicolon_after_source_location(source, &revert_statement.src).map(|i| i + 1)
        }
        Statement::TryStatement(try_statement) => {
            find_next_index_of_source_location(&try_statement.src)
        }
        Statement::UncheckedBlock(unchecked_block) => {
            find_next_index_of_source_location(&unchecked_block.src)
        }
        Statement::VariableDeclarationStatement(variable_declaration_statement) => {
            find_next_semicolon_after_source_location(source, &variable_declaration_statement.src)
                .map(|i| i + 1)
        }
        Statement::WhileStatement(while_statement) => {
            find_next_index_of_block_or_statement(source, &while_statement.body)
        }
    }
}
