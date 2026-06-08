import '@testing-library/jest-dom'
import { configure } from '@testing-library/react'

// react-markdown is lazy-loaded behind React.lazy + Suspense (fewd-0fq), so any
// view that renders recipe / notes / cook-step markdown only appears after the
// dynamic import resolves. RTL's findBy*/waitFor default budget is 1000ms — too
// tight for that chunk to load under full-suite concurrency (43 files in
// parallel, often racing a `cargo build` for I/O), which intermittently timed
// out tests whose content renders fine, just slowly (fewd-rpr: CookingView,
// RecipeMarkdown, RecipeDetailPage). This raises the async-utility budget to a
// generous-but-still-bounded 5s; a genuinely missing element still fails, just
// not before the lazy chunk has had a fair chance to mount. Stays well under the
// 15s per-test `testTimeout` so a truly hung render still surfaces.
configure({ asyncUtilTimeout: 5000 })
