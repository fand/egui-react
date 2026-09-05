// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

use accesskit::{Role, Toggled};
use accesskit_consumer::Node;
use core::fmt::Write as _;
use web_sys::HtmlElement;

use crate::filters::filter;

pub(crate) struct NodeWrapper<'a> {
    pub(crate) node: Node<'a>,
    /// Draw a green outline around every mirrored node.
    pub(crate) debug: bool,
}

impl NodeWrapper<'_> {
    fn role(&self) -> Option<String> {
        let role = self.node.role();
        match role {
            Role::Cell => Some("cell".into()),
            Role::Image => Some("img".into()),
            Role::Link => Some("link".into()),
            Role::Row => Some("row".into()),
            Role::ListItem => Some("listitem".into()),
            Role::TreeItem => Some("treeitem".into()),
            Role::ListBoxOption => Some("option".into()),
            Role::MenuItem | Role::MenuListOption => Some("menuitem".into()),
            Role::Paragraph => Some("paragraph".into()),
            Role::GenericContainer => Some("generic".into()),
            Role::CheckBox => Some("checkbox".into()),
            Role::RadioButton => Some("radio".into()),
            Role::TextInput => Some("textbox".into()),
            Role::Button | Role::DefaultButton => Some("button".into()),
            Role::RowHeader => Some("rowheader".into()),
            Role::ColumnHeader => Some("columnheader".into()),
            Role::RowGroup => Some("rowgroup".into()),
            Role::List => Some("list".into()),
            Role::Table => Some("table".into()),
            Role::Switch => Some("switch".into()),
            Role::Menu => Some("menu".into()),
            Role::MultilineTextInput => Some("textbox".into()),
            Role::SearchInput => Some("searchbox".into()),
            Role::DateInput
            | Role::DateTimeInput
            | Role::WeekInput
            | Role::MonthInput
            | Role::TimeInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput => Some("textbox".into()),
            Role::Alert => Some("alert".into()),
            Role::AlertDialog => Some("alertdialog".into()),
            Role::Application => Some("application".into()),
            Role::Article => Some("article".into()),
            Role::Banner => Some("banner".into()),
            Role::Blockquote => Some("blockquote".into()),
            Role::Caption => Some("caption".into()),
            Role::Code => Some("code".into()),
            Role::ComboBox | Role::EditableComboBox => Some("combobox".into()),
            Role::Complementary => Some("complementary".into()),
            Role::Comment => Some("comment".into()),
            Role::ContentDeletion => Some("deletion".into()),
            Role::ContentInsertion => Some("insertion".into()),
            Role::ContentInfo => Some("contentinfo".into()),
            Role::Definition => Some("definition".into()),
            Role::Dialog => Some("dialog".into()),
            Role::Document => Some("document".into()),
            Role::Emphasis => Some("emphasis".into()),
            Role::Feed => Some("feed".into()),
            Role::Figure => Some("figure".into()),
            Role::Footer => Some("contentinfo".into()),
            Role::Form => Some("form".into()),
            Role::Grid => Some("grid".into()),
            Role::Group => Some("group".into()),
            Role::Header => Some("banner".into()),
            Role::Heading => Some("heading".into()),
            Role::ListBox => Some("listbox".into()),
            Role::Log => Some("log".into()),
            Role::Main => Some("main".into()),
            Role::Mark => Some("mark".into()),
            Role::Marquee => Some("marquee".into()),
            Role::Math => Some("math".into()),
            Role::MenuBar => Some("menubar".into()),
            Role::MenuItemCheckBox => Some("menuitemcheckbox".into()),
            Role::MenuItemRadio => Some("menuitemradio".into()),
            Role::Meter => Some("meter".into()),
            Role::Navigation => Some("navigation".into()),
            Role::Note => Some("note".into()),
            Role::ProgressIndicator => Some("progressbar".into()),
            Role::RadioGroup => Some("radiogroup".into()),
            Role::Region => Some("region".into()),
            Role::ScrollBar => Some("scrollbar".into()),
            Role::Search => Some("search".into()),
            Role::Section => Some("section".into()),
            Role::Slider => Some("slider".into()),
            Role::SpinButton => Some("spinbutton".into()),
            Role::Splitter => Some("separator".into()),
            Role::Status => Some("status".into()),
            Role::Strong => Some("strong".into()),
            Role::Suggestion => Some("suggestion".into()),
            Role::Tab => Some("tab".into()),
            Role::TabList => Some("tablist".into()),
            Role::TabPanel => Some("tabpanel".into()),
            Role::Term => Some("term".into()),
            Role::Time => Some("time".into()),
            Role::Timer => Some("timer".into()),
            Role::Toolbar => Some("toolbar".into()),
            Role::Tooltip => Some("tooltip".into()),
            Role::Tree => Some("tree".into()),
            Role::TreeGrid => Some("treegrid".into()),
            Role::Window => Some("window".into()),
            Role::GraphicsDocument => Some("graphics-document".into()),
            Role::GraphicsObject => Some("graphics-object".into()),
            Role::GraphicsSymbol => Some("graphics-symbol".into()),
            Role::DocAbstract => Some("doc-abstract".into()),
            Role::DocAcknowledgements => Some("doc-acknowledgements".into()),
            Role::DocAfterword => Some("doc-afterword".into()),
            Role::DocAppendix => Some("doc-appendix".into()),
            Role::DocBackLink => Some("doc-backlink".into()),
            Role::DocBiblioEntry => Some("doc-biblioentry".into()),
            Role::DocBibliography => Some("doc-bibliography".into()),
            Role::DocBiblioRef => Some("doc-biblioref".into()),
            Role::DocChapter => Some("doc-chapter".into()),
            Role::DocColophon => Some("doc-colophon".into()),
            Role::DocConclusion => Some("doc-conclusion".into()),
            Role::DocCover => Some("doc-cover".into()),
            Role::DocCredit => Some("doc-credit".into()),
            Role::DocCredits => Some("doc-credits".into()),
            Role::DocDedication => Some("doc-dedication".into()),
            Role::DocEndnote => Some("doc-endnote".into()),
            Role::DocEndnotes => Some("doc-endnotes".into()),
            Role::DocEpigraph => Some("doc-epigraph".into()),
            Role::DocEpilogue => Some("doc-epilogue".into()),
            Role::DocErrata => Some("doc-errata".into()),
            Role::DocExample => Some("doc-example".into()),
            Role::DocFootnote => Some("doc-footnote".into()),
            Role::DocForeword => Some("doc-forward".into()),
            Role::DocGlossary => Some("doc-glossary".into()),
            Role::DocGlossRef => Some("doc-glossref".into()),
            Role::DocIndex => Some("doc-index".into()),
            Role::DocIntroduction => Some("doc-introduction".into()),
            Role::DocNoteRef => Some("doc-noteref".into()),
            Role::DocNotice => Some("doc-notice".into()),
            Role::DocPageBreak => Some("doc-pagebreak".into()),
            Role::DocPageFooter => Some("doc-pagefooter".into()),
            Role::DocPageHeader => Some("doc-pageheader".into()),
            Role::DocPageList => Some("doc-pagelist".into()),
            Role::DocPart => Some("doc-part".into()),
            Role::DocPreface => Some("doc-preface".into()),
            Role::DocPrologue => Some("doc-prologue".into()),
            Role::DocPullquote => Some("doc-pullquote".into()),
            Role::DocQna => Some("doc-qna".into()),
            Role::DocSubtitle => Some("doc-subtitle".into()),
            Role::DocTip => Some("doc-tip".into()),
            Role::DocToc => Some("doc-toc".into()),
            // Plain text with no role is merged into its neighbours by Safari
            // (flutter#166787), so say what it is.
            Role::Label => Some("paragraph".into()),
            _ => None,
        }
    }

    /// Where the mirror element goes, in the physical pixels the tree uses.
    ///
    /// The host scales the whole mirror back to CSS pixels in one go, so no
    /// division happens here.
    ///
    /// A node's element is nested inside its parent's, and an absolutely
    /// positioned parent is the containing block of its absolutely positioned
    /// children, so the offsets have to be relative to the nearest mirrored
    /// ancestor that has a box. Flutter's semantics layer does the same.
    fn style(&self) -> Option<String> {
        let mut style = String::from("position:absolute;overflow:visible;");
        match self.node.bounding_box() {
            Some(bounds) => {
                let (origin_x, origin_y) = ancestor_origin(&self.node);
                write!(
                    style,
                    "left:{}px;top:{}px;width:{}px;height:{}px;",
                    bounds.x0 - origin_x,
                    bounds.y0 - origin_y,
                    bounds.width(),
                    bounds.height()
                )
                .ok()?;
            }
            // No box of its own: sit on the ancestor's origin so that the
            // children below it keep their coordinates.
            None => style.push_str("left:0;top:0;"),
        }
        if self.debug {
            // An outline, not a border: a border would move the layout.
            style.push_str("outline:1px solid green;");
        }
        Some(style)
    }

    /// Focusable nodes are Tab stops, in tree order, which is DOM order.
    fn tabindex(&self) -> Option<String> {
        self.node.is_focusable(&filter).then(|| "0".into())
    }

    fn label(&self) -> Option<String> {
        self.node.label()
    }

    fn aria_label(&self) -> Option<String> {
        if self.node.role() == Role::Label {
            return None;
        }
        self.label()
    }

    fn text_content(&self) -> Option<String> {
        if self.node.role() != Role::Label {
            return None;
        }
        self.label()
    }

    fn aria_checked(&self) -> Option<String> {
        self.node.toggled().map(|value| match value {
            Toggled::False => "false".into(),
            Toggled::True => "true".into(),
            Toggled::Mixed => "mixed".into(),
        })
    }

    fn aria_valuemax(&self) -> Option<String> {
        self.node.max_numeric_value().map(|value| value.to_string())
    }

    fn aria_valuemin(&self) -> Option<String> {
        self.node.min_numeric_value().map(|value| value.to_string())
    }

    fn aria_valuenow(&self) -> Option<String> {
        self.node.numeric_value().map(|value| value.to_string())
    }

    fn aria_valuetext(&self) -> Option<String> {
        self.node.value()
    }
}

macro_rules! attributes {
    ($(($name:literal, $m:ident)),+) => {
        impl NodeWrapper<'_> {
            /// Rewrite only the position, for when the mirror's own settings
            /// changed rather than the tree.
            pub(crate) fn set_style(&self, element: &HtmlElement) {
                if let Some(style) = self.style().as_ref() {
                    let _ = element.set_attribute("style", style);
                }
            }

            pub(crate) fn set_all_attributes(&self, element: &HtmlElement) {
                $(let value = self.$m();
                if let Some(value) = value.as_ref() {
                    let _ = element.set_attribute($name, value);
                })*
                if let Some(text_content) = self.text_content().as_ref() {
                    element.set_text_content(Some(text_content));
                }
            }

            pub(crate) fn update_attributes(&self, element: &HtmlElement, old: &NodeWrapper<'_>) {
                $({
                    let old_value = old.$m();
                    let new_value = self.$m();
                    if old_value != new_value {
                        if let Some(value) = new_value.as_ref() {
                            let _ = element.set_attribute($name, value);
                        } else {
                            let _ = element.remove_attribute($name);
                        }
                    }
                })*
                let old_text_content = old.text_content();
                let new_text_content = self.text_content();
                if old_text_content != new_text_content {
                    element.set_text_content(new_text_content.as_deref());
                }
            }
        }
    };
}

/// The origin the node's own coordinates are written against: the nearest
/// mirrored ancestor that has a bounding box, or the mirror host.
fn ancestor_origin(node: &Node<'_>) -> (f64, f64) {
    let mut ancestor = node.filtered_parent(&filter);
    while let Some(current) = ancestor {
        if let Some(bounds) = current.bounding_box() {
            return (bounds.x0, bounds.y0);
        }
        ancestor = current.filtered_parent(&filter);
    }
    (0.0, 0.0)
}

attributes! {
    ("style", style),
    ("role", role),
    ("tabindex", tabindex),
    ("aria-label", aria_label),
    ("aria-checked", aria_checked),
    ("aria-valuemax", aria_valuemax),
    ("aria-valuemin", aria_valuemin),
    ("aria-valuenow", aria_valuenow),
    ("aria-valuetext", aria_valuetext)
}
