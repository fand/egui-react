// The default theme plus two components and one stylesheet.
//
// The navigation, the sidebar and the article layout are the default theme's,
// which is what readers of other Rust and Vue documentation already know. An
// example page is the exception: `ExamplePage` lays the content area out as
// the gallery did, the running example on the left and the code on the right.

import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import ExampleEmbed from './ExampleEmbed.vue'
import ExamplePage from './ExamplePage.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Global, because every example page uses one and a page should be
    // markdown with one tag in it, not markdown with an import block on top.
    app.component('ExampleEmbed', ExampleEmbed)
    app.component('ExamplePage', ExamplePage)
  }
} satisfies Theme
