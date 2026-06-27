<script lang="ts">
  import "../global.css";
  import ToastContainer from '../components/ui/ToastContainer.svelte';
  import UpdateModal from '../components/ui/UpdateModal.svelte';

  let { children } = $props();
</script>

{@render children()}
<ToastContainer />
<UpdateModal />
