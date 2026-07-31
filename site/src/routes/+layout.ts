// Prerender the page so search engines see the full title, H1, and body in the
// initial HTML (not after JS hydration). The locale is still chosen client-side;
// the prerendered output is English (the SEO target language) and hydrates into
// the user's locale on load. See svelte.config.js adapter-static.
export const prerender = true
