// The default theme plus one component and one stylesheet.
//
// Nothing about the navigation, the sidebar or the article layout is ours: the
// site is a documentation site and the default theme is what readers of other
// Rust and Vue documentation already know.

import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import ExampleEmbed from './ExampleEmbed.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Global, because every example page uses it and a page should be markdown
    // with one tag in it, not markdown with an import block on top.
    app.component('ExampleEmbed', ExampleEmbed)
  }
} satisfies Theme
