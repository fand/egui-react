//! Paint attributes: [`PaintStyle`], the look of a box.
//!
//! [`layout`](crate::layout) says where a box is; this says what it looks like.
//! It is a field of [`ItemStyle`](crate::layout::ItemStyle), so every element
//! takes it through the one `style` prop it already has, and `rsx!` spells it
//! with the shorthands `bg` `border` `radius` `shadow` `custom_shadow`
//! `opacity`.
//!
//! Nothing here draws. The engine knows the rect of a node only after the
//! layout is solved, so it claims a shape slot in draw order and fills it with
//! what these helpers return once the frame's layout is final. That is why the
//! helpers take a rect and hand back a [`Shape`]: the same three shapes serve
//! the taffy path and the lite path.

use egui::epaint::RectShape;
use egui::{Color32, Rect, Shadow, Shape, Stroke, StrokeKind, Visuals};

/// How a box looks: background, border, corner radius, shadow and opacity.
///
/// Every field is optional and `Default` paints nothing, so an element that
/// nobody styled costs no shapes at all.
///
/// The three shapes all sit on the node's *border box* — its whole box, not
/// the content rect — and share one `radius`. A `border` of width `n` is also
/// reserved by the layout (`ItemStyle::to_taffy` sets taffy's `border`), so the
/// stroke is drawn `StrokeKind::Inside` and lands exactly in the band the
/// children were kept out of.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintStyle {
    /// Background colour, filled behind the children or the widget.
    pub bg: Option<Color32>,
    /// Border stroke, drawn on top of them and reserved by the layout.
    pub border: Option<Stroke>,
    /// Corner radius, shared by the shadow, the background and the border.
    pub radius: Option<f32>,
    /// Cast the theme's `Visuals::window_shadow`.
    pub shadow: bool,
    /// Cast this shadow instead, whatever `shadow` says.
    pub custom_shadow: Option<Shadow>,
    /// Multiply the opacity of the node and everything under it.
    pub opacity: Option<f32>,
}

impl PaintStyle {
    /// Set the background colour.
    #[must_use]
    pub fn bg(mut self, v: impl Into<Color32>) -> Self {
        self.bg = Some(v.into());
        self
    }

    /// Set the border stroke. The layout reserves its width.
    #[must_use]
    pub fn border(mut self, v: impl Into<Stroke>) -> Self {
        self.border = Some(v.into());
        self
    }

    /// Set the corner radius of all three shapes.
    #[must_use]
    pub fn radius(mut self, v: f32) -> Self {
        self.radius = Some(v);
        self
    }

    /// Cast the theme's window shadow, or stop casting it.
    #[must_use]
    pub fn shadow(mut self, v: bool) -> Self {
        self.shadow = v;
        self
    }

    /// Cast this shadow instead of the theme's.
    #[must_use]
    pub fn custom_shadow(mut self, v: Shadow) -> Self {
        self.custom_shadow = Some(v);
        self
    }

    /// Multiply the opacity of this node and everything under it.
    #[must_use]
    pub fn opacity(mut self, v: f32) -> Self {
        self.opacity = Some(v);
        self
    }

    /// Whether this style does nothing, so the engine can skip the node.
    ///
    /// `radius` alone does not count: it only rounds shapes that another field
    /// asks for. `opacity` does, even though it draws nothing itself, because
    /// the engine still has to set it around the children.
    pub fn is_none(&self) -> bool {
        self.bg.is_none()
            && self.border.is_none()
            && !self.shadow
            && self.custom_shadow.is_none()
            && self.opacity.is_none()
    }

    /// The corner radius the three shapes share, `0.0` when unset.
    fn corner_radius(&self) -> f32 {
        self.radius.unwrap_or(0.0)
    }

    /// The shadow shape under `rect`, if this style casts one.
    ///
    /// `custom_shadow` wins over `shadow`; `shadow` alone is the theme's
    /// window shadow, which is why this needs the `Visuals`. `Shadow::as_shape`
    /// offsets and expands the rect itself, so `rect` is the plain border box.
    pub fn shadow_shape(&self, rect: Rect, visuals: &Visuals) -> Option<Shape> {
        let shadow = self
            .custom_shadow
            .or_else(|| self.shadow.then_some(visuals.window_shadow))?;
        Some(shadow.as_shape(rect, self.corner_radius()).into())
    }

    /// The background shape filling `rect`, if there is a `bg`.
    pub fn bg_shape(&self, rect: Rect) -> Option<Shape> {
        let bg = self.bg?;
        Some(RectShape::filled(rect, self.corner_radius(), bg).into())
    }

    /// The border shape around `rect`, if there is a `border`.
    ///
    /// `StrokeKind::Inside`: the layout already reserved the stroke's width
    /// inside the border box, so drawing it there costs no extra room and two
    /// neighbours with borders do not overlap.
    pub fn border_shape(&self, rect: Rect) -> Option<Shape> {
        let border = self.border?;
        Some(RectShape::stroke(rect, self.corner_radius(), border, StrokeKind::Inside).into())
    }
}
