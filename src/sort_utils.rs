/// Sort direction for table columns.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Desc,
    Asc,
}

impl SortDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "asc" => Self::Asc,
            _ => Self::Desc,
        }
    }

    pub fn sql(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// Trait for column enums. Each sortable table defines its own column enum
/// implementing this trait.
pub trait SortableColumn: Sized + Default + Clone + PartialEq {
    /// Parse column name from query string parameter.
    fn from_str(s: &str) -> Option<Self>;

    /// Convert column to query string parameter value.
    fn as_str(&self) -> &'static str;

    /// SQL expression for ORDER BY clause (e.g., "e.date", "e.amount_cents").
    fn sql_expression(&self) -> &'static str;
}

/// Trait for filter params that support sorting (similar to DateFilterable).
pub trait Sortable {
    fn sort_by(&self) -> Option<&String>;
    fn sort_dir(&self) -> Option<&String>;

    /// Resolve sort parameters into a TableSort config.
    fn resolve_sort<C: SortableColumn>(&self) -> TableSort<C> {
        let column = self
            .sort_by()
            .and_then(|s| C::from_str(s))
            .unwrap_or_default();

        let direction = self
            .sort_dir()
            .map(|s| SortDirection::from_str(s))
            .unwrap_or_default();

        TableSort { column, direction }
    }

    /// Generate query string for sort parameters (e.g., "sort=date&dir=desc").
    fn sort_query_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(sort) = self.sort_by() {
            parts.push(format!("sort={}", sort));
        }
        if let Some(dir) = self.sort_dir() {
            parts.push(format!("dir={}", dir));
        }
        parts.join("&")
    }
}

/// Sort configuration passed to templates.
#[derive(Debug, Clone)]
pub struct TableSort<C: SortableColumn> {
    pub column: C,
    pub direction: SortDirection,
}

impl<C: SortableColumn> TableSort<C> {
    /// Generate SQL ORDER BY expression (e.g., "e.date DESC").
    pub fn sql_order_by(&self) -> String {
        format!("{} {}", self.column.sql_expression(), self.direction.sql())
    }

    /// Check if this column is currently being sorted.
    pub fn is_active(&self, col: &C) -> bool {
        &self.column == col
    }

    /// Get the direction to use when clicking a column header.
    /// If already sorted by this column, toggle direction; otherwise use default (Desc).
    pub fn next_direction_for(&self, col: &C) -> SortDirection {
        if self.is_active(col) {
            self.direction.toggle()
        } else {
            SortDirection::Desc
        }
    }

    /// Get sort indicator for a column header ("▲", "▼", or "").
    pub fn indicator(&self, col: &C) -> &'static str {
        if self.is_active(col) {
            match self.direction {
                SortDirection::Asc => "▲",
                SortDirection::Desc => "▼",
            }
        } else {
            ""
        }
    }

    /// Generate query string for current sort state.
    pub fn query_string(&self) -> String {
        format!("sort={}&dir={}", self.column.as_str(), self.direction.as_str())
    }

    /// Generate query string for sorting by a specific column.
    pub fn query_string_for(&self, col: &C) -> String {
        let dir = self.next_direction_for(col);
        format!("sort={}&dir={}", col.as_str(), dir.as_str())
    }

    // String-based methods for easier use in templates

    /// Check if sorting by the given column name (string version for templates).
    pub fn is_active_str(&self, col_name: &str) -> bool {
        C::from_str(col_name).is_some_and(|c| self.is_active(&c))
    }

    /// Get sort indicator for a column by name (string version for templates).
    pub fn indicator_str(&self, col_name: &str) -> &'static str {
        match C::from_str(col_name) {
            Some(col) => self.indicator(&col),
            None => "",
        }
    }

    /// Generate query string for sorting by column name (string version for templates).
    pub fn query_string_for_str(&self, col_name: &str) -> String {
        match C::from_str(col_name) {
            Some(col) => self.query_string_for(&col),
            None => String::new(),
        }
    }
}

impl<C: SortableColumn> Default for TableSort<C> {
    fn default() -> Self {
        Self {
            column: C::default(),
            direction: SortDirection::default(),
        }
    }
}
