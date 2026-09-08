//! Layout attributes: [`Length`], [`ItemStyle`] and [`ContainerStyle`].
//!
//! These are the types every element accepts as layout props. They exist so
//! that `rsx!` can hand string literals (`justify="space-between"`) and numbers
//! (`gap={8}`) straight to a setter, and so that the conversion to
//! [`taffy::Style`] lives in one place.

/// A length in a layout attribute.
///
/// `Percent` holds a fraction, like taffy: `Length::Percent(0.5)` is `50%`.
/// The `From<&str>` impl parses the CSS-ish spellings (`"50%"`, `"auto"`,
/// `"12px"`, `"12"`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// An absolute length in egui points.
    Px(f32),
    /// A fraction of the containing block (`0.5` is `50%`).
    Percent(f32),
    /// Sized by the layout algorithm.
    #[default]
    Auto,
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Self::Px(v)
    }
}

impl From<i32> for Length {
    fn from(v: i32) -> Self {
        Self::Px(v as f32)
    }
}

impl From<&str> for Length {
    /// # Panics
    /// Panics on anything that is not `"auto"`, `"<n>%"`, `"<n>px"` or `"<n>"`.
    fn from(s: &str) -> Self {
        let t = s.trim();
        if t == "auto" {
            return Self::Auto;
        }
        if let Some(n) = t.strip_suffix('%') {
            return Self::Percent(parse_number(n, s) / 100.0);
        }
        let n = t.strip_suffix("px").unwrap_or(t);
        Self::Px(parse_number(n, s))
    }
}

fn parse_number(n: &str, whole: &str) -> f32 {
    n.trim().parse::<f32>().unwrap_or_else(|_| {
        panic!("egui-react: cannot parse {whole:?} as a length (expected \"auto\", \"50%\", \"12px\" or \"12\")")
    })
}

impl Length {
    /// As a taffy `Dimension` (`width` / `height` / `flex-basis`).
    pub fn to_dimension(self) -> taffy::Dimension {
        match self {
            Self::Px(v) => taffy::Dimension::length(v),
            Self::Percent(v) => taffy::Dimension::percent(v),
            Self::Auto => taffy::Dimension::auto(),
        }
    }

    /// As a taffy `LengthPercentageAuto` (`margin`).
    pub fn to_length_percentage_auto(self) -> taffy::LengthPercentageAuto {
        match self {
            Self::Px(v) => taffy::LengthPercentageAuto::length(v),
            Self::Percent(v) => taffy::LengthPercentageAuto::percent(v),
            Self::Auto => taffy::LengthPercentageAuto::auto(),
        }
    }

    /// As a taffy `LengthPercentage` (`padding`). `Auto` becomes zero.
    pub fn to_length_percentage(self) -> taffy::LengthPercentage {
        match self {
            Self::Px(v) => taffy::LengthPercentage::length(v),
            Self::Percent(v) => taffy::LengthPercentage::percent(v),
            Self::Auto => taffy::LengthPercentage::length(0.0),
        }
    }
}

/// Build a layout enum together with its `From<&str>` parser.
macro_rules! str_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $text:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
        }

        impl $name {
            /// Parse the CSS-ish spelling, returning `None` if unknown.
            pub fn parse(s: &str) -> Option<Self> {
                match s.trim() {
                    $( $text => Some(Self::$variant), )*
                    _ => None,
                }
            }
        }

        impl From<&str> for $name {
            /// # Panics
            /// Panics on an unknown spelling.
            fn from(s: &str) -> Self {
                Self::parse(s).unwrap_or_else(|| {
                    panic!(
                        "egui-react: {:?} is not a valid {} (expected one of: {})",
                        s,
                        stringify!($name),
                        [$($text),*].join(", "),
                    )
                })
            }
        }
    };
}

str_enum! {
    /// `flex-direction`.
    pub enum Direction {
        #[default] Row = "row",
        Column = "column",
        RowReverse = "row-reverse",
        ColumnReverse = "column-reverse",
    }
}

str_enum! {
    /// `justify-content` / `align-content`. `Normal` leaves it unset.
    pub enum Justify {
        #[default] Normal = "normal",
        Start = "start",
        End = "end",
        FlexStart = "flex-start",
        FlexEnd = "flex-end",
        Center = "center",
        Stretch = "stretch",
        SpaceBetween = "space-between",
        SpaceEvenly = "space-evenly",
        SpaceAround = "space-around",
    }
}

str_enum! {
    /// `align-items` / `align-self`. `Normal` leaves it unset.
    pub enum Align {
        #[default] Normal = "normal",
        Start = "start",
        End = "end",
        FlexStart = "flex-start",
        FlexEnd = "flex-end",
        Center = "center",
        Baseline = "baseline",
        Stretch = "stretch",
    }
}

str_enum! {
    /// `display`.
    pub enum Display {
        #[default] Flex = "flex",
        Grid = "grid",
        Block = "block",
        None = "none",
    }
}

/// `align-self`, which taffy spells with the same type as `align-items`.
pub type AlignSelf = Align;

impl Direction {
    /// The taffy equivalent.
    pub fn to_taffy(self) -> taffy::FlexDirection {
        match self {
            Self::Row => taffy::FlexDirection::Row,
            Self::Column => taffy::FlexDirection::Column,
            Self::RowReverse => taffy::FlexDirection::RowReverse,
            Self::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        }
    }
}

impl Justify {
    /// The taffy equivalent, or `None` for `Normal`.
    pub fn to_taffy(self) -> Option<taffy::AlignContent> {
        Some(match self {
            Self::Normal => return None,
            Self::Start => taffy::AlignContent::Start,
            Self::End => taffy::AlignContent::End,
            Self::FlexStart => taffy::AlignContent::FlexStart,
            Self::FlexEnd => taffy::AlignContent::FlexEnd,
            Self::Center => taffy::AlignContent::Center,
            Self::Stretch => taffy::AlignContent::Stretch,
            Self::SpaceBetween => taffy::AlignContent::SpaceBetween,
            Self::SpaceEvenly => taffy::AlignContent::SpaceEvenly,
            Self::SpaceAround => taffy::AlignContent::SpaceAround,
        })
    }
}

impl Align {
    /// The taffy equivalent, or `None` for `Normal`.
    pub fn to_taffy(self) -> Option<taffy::AlignItems> {
        Some(match self {
            Self::Normal => return None,
            Self::Start => taffy::AlignItems::Start,
            Self::End => taffy::AlignItems::End,
            Self::FlexStart => taffy::AlignItems::FlexStart,
            Self::FlexEnd => taffy::AlignItems::FlexEnd,
            Self::Center => taffy::AlignItems::Center,
            Self::Baseline => taffy::AlignItems::Baseline,
            Self::Stretch => taffy::AlignItems::Stretch,
        })
    }
}

impl Display {
    /// The taffy equivalent.
    pub fn to_taffy(self) -> taffy::Display {
        match self {
            Self::Flex => taffy::Display::Flex,
            Self::Grid => taffy::Display::Grid,
            Self::Block => taffy::Display::Block,
            Self::None => taffy::Display::None,
        }
    }
}

/// The layout attributes every element accepts, as a flex/grid *item*.
///
/// Every setter takes `impl Into<Length>`, so `.w(120.0)`, `.w("50%")` and
/// `.w(Length::Auto)` all work; `rsx!` passes literals straight through.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemStyle {
    /// `width`.
    pub w: Option<Length>,
    /// `height`.
    pub h: Option<Length>,
    /// `min-width`.
    pub min_w: Option<Length>,
    /// `min-height`.
    pub min_h: Option<Length>,
    /// `max-width`.
    pub max_w: Option<Length>,
    /// `max-height`.
    pub max_h: Option<Length>,
    /// `flex-grow`.
    pub grow: Option<f32>,
    /// `flex-shrink`.
    pub shrink: Option<f32>,
    /// `flex-basis`.
    pub basis: Option<Length>,
    /// `align-self`.
    pub align_self: Option<AlignSelf>,
    /// `margin` (all sides).
    pub m: Option<Length>,
    /// `margin-inline`.
    pub mx: Option<Length>,
    /// `margin-block`.
    pub my: Option<Length>,
    /// `margin-top`.
    pub mt: Option<Length>,
    /// `margin-right`.
    pub mr: Option<Length>,
    /// `margin-bottom`.
    pub mb: Option<Length>,
    /// `margin-left`.
    pub ml: Option<Length>,
    /// `padding` (all sides).
    pub p: Option<Length>,
    /// `padding-inline`.
    pub px: Option<Length>,
    /// `padding-block`.
    pub py: Option<Length>,
    /// `padding-top`.
    pub pt: Option<Length>,
    /// `padding-right`.
    pub pr: Option<Length>,
    /// `padding-bottom`.
    pub pb: Option<Length>,
    /// `padding-left`.
    pub pl: Option<Length>,
    /// `grid-column: span N`.
    pub col_span: Option<u16>,
    /// `grid-row: span N`.
    pub row_span: Option<u16>,
}

macro_rules! length_setters {
    ($( $(#[$meta:meta])* $name:ident ),* $(,)?) => {
        $(
            $(#[$meta])*
            #[must_use]
            pub fn $name(mut self, v: impl Into<Length>) -> Self {
                self.$name = Some(v.into());
                self
            }
        )*
    };
}

impl ItemStyle {
    length_setters!(
        /// Set `width`.
        w,
        /// Set `height`.
        h,
        /// Set `min-width`.
        min_w,
        /// Set `min-height`.
        min_h,
        /// Set `max-width`.
        max_w,
        /// Set `max-height`.
        max_h,
        /// Set `flex-basis`.
        basis,
        /// Set the margin on all four sides.
        m,
        /// Set the left and right margin.
        mx,
        /// Set the top and bottom margin.
        my,
        /// Set the top margin.
        mt,
        /// Set the right margin.
        mr,
        /// Set the bottom margin.
        mb,
        /// Set the left margin.
        ml,
        /// Set the padding on all four sides.
        p,
        /// Set the left and right padding.
        px,
        /// Set the top and bottom padding.
        py,
        /// Set the top padding.
        pt,
        /// Set the right padding.
        pr,
        /// Set the bottom padding.
        pb,
        /// Set the left padding.
        pl,
    );

    /// Set `flex-grow`.
    #[must_use]
    pub fn grow(mut self, v: f32) -> Self {
        self.grow = Some(v);
        self
    }

    /// Set `flex-shrink`.
    #[must_use]
    pub fn shrink(mut self, v: f32) -> Self {
        self.shrink = Some(v);
        self
    }

    /// Set `align-self`.
    #[must_use]
    pub fn align_self(mut self, v: impl Into<AlignSelf>) -> Self {
        self.align_self = Some(v.into());
        self
    }

    /// Set how many grid columns this item spans.
    #[must_use]
    pub fn col_span(mut self, v: u16) -> Self {
        self.col_span = Some(v);
        self
    }

    /// Set how many grid rows this item spans.
    #[must_use]
    pub fn row_span(mut self, v: u16) -> Self {
        self.row_span = Some(v);
        self
    }

    /// The taffy style of this item, with container properties left default.
    pub fn to_taffy(&self) -> taffy::Style {
        let dim = |v: Option<Length>| {
            v.map(Length::to_dimension)
                .unwrap_or(taffy::Dimension::auto())
        };
        let mut style = taffy::Style {
            size: taffy::Size {
                width: dim(self.w),
                height: dim(self.h),
            },
            min_size: taffy::Size {
                width: dim(self.min_w),
                height: dim(self.min_h),
            },
            max_size: taffy::Size {
                width: dim(self.max_w),
                height: dim(self.max_h),
            },
            margin: resolve_rect(
                [self.m, self.mx, self.my, self.mt, self.mr, self.mb, self.ml],
                Length::to_length_percentage_auto,
                taffy::LengthPercentageAuto::length(0.0),
            ),
            padding: resolve_rect(
                [self.p, self.px, self.py, self.pt, self.pr, self.pb, self.pl],
                Length::to_length_percentage,
                taffy::LengthPercentage::length(0.0),
            ),
            ..Default::default()
        };
        if let Some(grow) = self.grow {
            style.flex_grow = grow;
        }
        if let Some(shrink) = self.shrink {
            style.flex_shrink = shrink;
        }
        if let Some(basis) = self.basis {
            style.flex_basis = basis.to_dimension();
        }
        if let Some(align_self) = self.align_self {
            style.align_self = align_self.to_taffy();
        }
        if let Some(span) = self.col_span {
            style.grid_column = taffy::style_helpers::span(span);
        }
        if let Some(span) = self.row_span {
            style.grid_row = taffy::style_helpers::span(span);
        }
        style
    }
}

/// Resolve `[all, x, y, top, right, bottom, left]` into a taffy `Rect`.
///
/// The shorthand order is `all` -> `x` / `y` -> the individual side, so the
/// most specific value written wins.
fn resolve_rect<T: Copy>(
    [all, x, y, top, right, bottom, left]: [Option<Length>; 7],
    convert: impl Fn(Length) -> T,
    zero: T,
) -> taffy::Rect<T> {
    let pick = |specific: Option<Length>, axis: Option<Length>| {
        specific.or(axis).or(all).map(&convert).unwrap_or(zero)
    };
    taffy::Rect {
        top: pick(top, y),
        right: pick(right, x),
        bottom: pick(bottom, y),
        left: pick(left, x),
    }
}

/// The `gap` prop of a container: one value for both axes, or one per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gap {
    /// Space between columns, in points.
    pub column: f32,
    /// Space between rows, in points.
    pub row: f32,
}

impl From<f32> for Gap {
    fn from(v: f32) -> Self {
        Self { column: v, row: v }
    }
}

impl From<i32> for Gap {
    fn from(v: i32) -> Self {
        Self::from(v as f32)
    }
}

impl From<(f32, f32)> for Gap {
    fn from((column, row): (f32, f32)) -> Self {
        Self { column, row }
    }
}

/// The layout attributes of a container (`<View>`), as a flex/grid *parent*.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainerStyle {
    /// `display`.
    pub display: Display,
    /// `flex-direction`.
    pub direction: Direction,
    /// `flex-wrap`.
    pub wrap: bool,
    /// `justify-content`.
    pub justify: Justify,
    /// `align-items`.
    pub align: Align,
    /// `align-content`.
    pub align_content: Option<Justify>,
    /// `column-gap` and `row-gap`, in points.
    pub gap: (f32, f32),
    /// Number of equal-width grid columns, for `display="grid"`.
    pub cols: Option<u16>,
}

impl ContainerStyle {
    /// Set `display`.
    #[must_use]
    pub fn display(mut self, v: impl Into<Display>) -> Self {
        self.display = v.into();
        self
    }

    /// Set `flex-direction`.
    #[must_use]
    pub fn direction(mut self, v: impl Into<Direction>) -> Self {
        self.direction = v.into();
        self
    }

    /// Set `flex-wrap`.
    #[must_use]
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }

    /// Set `justify-content`.
    #[must_use]
    pub fn justify(mut self, v: impl Into<Justify>) -> Self {
        self.justify = v.into();
        self
    }

    /// Set `align-items`.
    #[must_use]
    pub fn align(mut self, v: impl Into<Align>) -> Self {
        self.align = v.into();
        self
    }

    /// Set `align-content`.
    #[must_use]
    pub fn align_content(mut self, v: impl Into<Justify>) -> Self {
        self.align_content = Some(v.into());
        self
    }

    /// Set the column and row gap.
    #[must_use]
    pub fn gap(mut self, v: impl Into<Gap>) -> Self {
        let gap = v.into();
        self.gap = (gap.column, gap.row);
        self
    }

    /// Set the column and row gap separately.
    #[must_use]
    pub fn gaps(mut self, column: f32, row: f32) -> Self {
        self.gap = (column, row);
        self
    }

    /// Set the number of equal-width grid columns.
    #[must_use]
    pub fn cols(mut self, v: u16) -> Self {
        self.cols = Some(v);
        self
    }

    /// The taffy style of a node that is both this container and `item`.
    ///
    /// A taffy node carries its own item properties and its children's
    /// container properties in one `Style`, so the two halves are merged here.
    pub fn merge(&self, item: &ItemStyle) -> taffy::Style {
        let mut style = item.to_taffy();
        style.display = self.display.to_taffy();
        style.flex_direction = self.direction.to_taffy();
        style.flex_wrap = if self.wrap {
            taffy::FlexWrap::Wrap
        } else {
            taffy::FlexWrap::NoWrap
        };
        style.justify_content = self.justify.to_taffy();
        style.align_items = self.align.to_taffy();
        style.align_content = self.align_content.and_then(Justify::to_taffy);
        style.gap = taffy::Size {
            width: taffy::LengthPercentage::length(self.gap.0),
            height: taffy::LengthPercentage::length(self.gap.1),
        };
        if self.display == Display::Grid
            && let Some(cols) = self.cols
        {
            style.grid_template_columns = taffy::style_helpers::evenly_sized_tracks(cols);
        }
        style
    }
}

/// The taffy style of the root container: a column that fills the window.
///
/// `reserve_available_space` tells the layout engine how much room there is,
/// but it leaves the root node's own `size` at `auto`, so taffy would size that
/// node by its content. Two things go wrong then. A `<View grow={1.0}
/// justify="center">` child finds no free space to grow into and nothing to be
/// centred in, so the app sits in the window's top-left corner. And a child
/// too wide to fit never has to shrink, because a content-sized parent simply
/// grows with it and there is no overflow to resolve — the row runs off the
/// right edge instead of `grow` and `flex-shrink` sharing out what there is.
///
/// So both sides are fixed at 100%: a window is exactly as big as it is. The
/// height used to be only a *minimum* of 100%, so that a column taller than
/// the window would lay out at its own height. That let a `leaf_fill` (a
/// `<ScrollArea>`, a `<VirtualList>`) push the root past the window: such a
/// leaf reports the whole root height as its content size, so a column of
/// "a header, then a list that fills the rest" measured as header plus window,
/// and the root grew to fit — the list's last rows sat below the window edge.
/// With a definite height the header keeps its content height (a flex item's
/// automatic minimum) and the list gets what is left. Content taller than the
/// window still overflows it rather than being shrunk, for the same reason,
/// and belongs in a `ScrollArea` as it always did.
///
/// Exposed so a runner, a test or an element that roots a tree of its own
/// (`<Overlay>`) all use the same style.
pub fn root_style() -> taffy::Style {
    ContainerStyle::default()
        .direction("column")
        .merge(&ItemStyle::default().w("100%").h("100%"))
}
