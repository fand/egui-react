<script setup lang="ts">
// The hero mark, inlined so a script can reach its animations: an `img` hides
// them. While the pointer is over it (or a finger is on it) the electrons spin
// faster. The rate eases toward its target frame by frame; setting
// `playbackRate` keeps each ball where it is, where changing the CSS duration
// would make them jump.
import { data as svg } from './hero-logo.data.js'

let rate = 1
let target = 1
let raf = 0

function speed(el: EventTarget | null, to: number) {
  target = to
  if (raf) return
  let last: number | undefined
  raf = requestAnimationFrame(function step(now) {
    // Close about two thirds of the gap every 200ms, whatever the frame rate.
    rate = target + (rate - target) * Math.exp(((last ?? now) - now) / 200)
    last = now
    if (Math.abs(target - rate) < 0.01) rate = target
    for (const a of (el as Element).getAnimations({ subtree: true })) a.playbackRate = rate
    raf = rate === target ? 0 : requestAnimationFrame(step)
  })
}
</script>

<template>
  <div
    class="image-src hero-logo"
    role="img"
    aria-label="atomic ferris"
    v-html="svg"
    @pointerenter="speed($event.currentTarget, 10)"
    @pointerleave="speed($event.currentTarget, 1)"
  />
</template>

<style scoped>
.hero-logo {
  width: 100%;
  height: 100%;
  -webkit-touch-callout: none;
  user-select: none;
}

.hero-logo :deep(svg) {
  width: 100%;
  height: 100%;
}
</style>
