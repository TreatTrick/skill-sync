<script lang="ts">
  import '../app.css'
  import Footer from '../lib/components/Footer.svelte'
  import Header from '../lib/components/Header.svelte'

  let { children } = $props()
</script>

<div class="flex min-h-screen flex-col bg-background text-foreground">
  <Header />
  <main class="flex-1">
    {@render children()}
  </main>
  <Footer />
</div>
