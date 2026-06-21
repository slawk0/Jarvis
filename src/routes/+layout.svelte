<script lang="ts">
  import "../global.css";
  import ToastContainer from '../components/ui/ToastContainer.svelte';

  let { children } = $props();
</script>

{@render children()}
<ToastContainer />
