// The example pages, in the gallery's order (`EXAMPLES` in
// examples/gallery/src/lib.rs). The sidebar, the home page's redirect and the
// Examples page are all built from this, so a page that is added or renamed
// needs no second edit.
//
// A file of its own, with no imports, because both `.vitepress/config.ts`
// (Node) and the data loaders read it.
export const EXAMPLE_ORDER = [
  'notes',
  'board',
  'patch',
  'spreadsheet',
  'counter',
  'todo',
  'form',
  'theme',
  'clock',
  'custom-hook',
  'escape-hatch',
  'shader',
  'list-10k',
  'layout',
  'styles',
  'fetch',
  'font'
]
