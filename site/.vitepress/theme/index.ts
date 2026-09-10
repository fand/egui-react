// The default theme plus four components and one stylesheet.
//
// The navigation, the sidebar and the article layout are the default theme's,
// which is what readers of other Rust and Vue documentation already know. An
// example page is the exception: `ExamplePage` lays the content area out as
// the gallery did, the running example on the left and the code on the right.

import { h } from 'vue'
import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import ExampleEmbed from './ExampleEmbed.vue'
import ExamplePage from './ExamplePage.vue'
import ExampleCards from './ExampleCards.vue'
import HeroLogo from './HeroLogo.vue'
import './custom.css'

export default {
  extends: DefaultTheme,
  // The hero's `image` in the frontmatter still turns the image column on;
  // this slot draws the mark inline in its place.
  Layout: () => h(DefaultTheme.Layout, null, { 'home-hero-image': () => h(HeroLogo) }),
  enhanceApp({ app }) {
    // Global, because every example page uses one and a page should be
    // markdown with one tag in it, not markdown with an import block on top.
    app.component('ExampleEmbed', ExampleEmbed)
    app.component('ExamplePage', ExamplePage)
    app.component('ExampleCards', ExampleCards)
  }
} satisfies Theme
