//! [`TextOptions`] — knobs for the layout-preserving text extractor.

/// Options that control layout-preserving text extraction.
///
/// Defaults match pdfplumber:
/// - `x_tolerance = 3.0`
/// - `y_tolerance = 3.0`
/// - `x_density   = 7.25` points per virtual column
/// - `y_density   = 13.0` points per virtual line
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// Maximum horizontal gap between adjacent glyphs of the same word.
    pub x_tolerance: f32,
    /// Maximum vertical difference for glyphs to be on the same line.
    pub y_tolerance: f32,
    /// When `Some(r)`, x-tolerance becomes `r * char.size` (dynamic).
    pub x_tolerance_ratio: Option<f32>,
    /// Points per virtual monospaced column in the output grid.
    pub x_density: f32,
    /// Points per virtual line in the output grid.
    pub y_density: f32,
    /// If `true`, keep literal blank characters from the PDF rather than
    /// treating them as word separators.
    pub keep_blank_chars: bool,
    /// If `true`, walk chars in stream order (do not sort by position).
    pub use_text_flow: bool,
    /// If `true`, expand multi-codepoint ligatures.
    pub expand_ligatures: bool,
}

impl TextOptions {
    /// Pdfplumber's default extraction settings.
    pub fn pdfplumber_defaults() -> Self {
        Self {
            x_tolerance: 3.0,
            y_tolerance: 3.0,
            x_tolerance_ratio: None,
            x_density: 7.25,
            y_density: 13.0,
            keep_blank_chars: false,
            use_text_flow: false,
            expand_ligatures: false,
        }
    }

    /// Begin a fluent builder.
    pub fn builder() -> TextOptionsBuilder {
        TextOptionsBuilder { inner: Self::pdfplumber_defaults() }
    }
}

impl Default for TextOptions {
    fn default() -> Self {
        Self::pdfplumber_defaults()
    }
}

/// Builder for [`TextOptions`].
#[derive(Debug, Clone)]
pub struct TextOptionsBuilder {
    inner: TextOptions,
}

impl TextOptionsBuilder {
    /// Set the horizontal gap tolerance (points).
    pub fn x_tolerance(mut self, v: f32) -> Self {
        self.inner.x_tolerance = v;
        self
    }
    /// Set the vertical line tolerance (points).
    pub fn y_tolerance(mut self, v: f32) -> Self {
        self.inner.y_tolerance = v;
        self
    }
    /// Set the dynamic x-tolerance ratio (relative to font size).
    pub fn x_tolerance_ratio(mut self, v: Option<f32>) -> Self {
        self.inner.x_tolerance_ratio = v;
        self
    }
    /// Set the virtual column width.
    pub fn x_density(mut self, v: f32) -> Self {
        self.inner.x_density = v;
        self
    }
    /// Set the virtual line height.
    pub fn y_density(mut self, v: f32) -> Self {
        self.inner.y_density = v;
        self
    }
    /// Keep literal blanks instead of using them as separators.
    pub fn keep_blank_chars(mut self, v: bool) -> Self {
        self.inner.keep_blank_chars = v;
        self
    }
    /// Preserve PDF stream order instead of sorting by position.
    pub fn use_text_flow(mut self, v: bool) -> Self {
        self.inner.use_text_flow = v;
        self
    }
    /// Expand ligatures (e.g. `ﬁ → fi`).
    pub fn expand_ligatures(mut self, v: bool) -> Self {
        self.inner.expand_ligatures = v;
        self
    }
    /// Build the final [`TextOptions`].
    pub fn build(self) -> TextOptions {
        self.inner
    }
}
