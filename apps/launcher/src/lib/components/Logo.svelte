<script lang="ts">
  // The Age of Decomp mark: a carved stone seal whose rings break open into circuit traces (original artwork made for
  // this project with codex-cli's image generation, cleaned in docs/assets/icon.png; the app icons come from it).
  import icon from '../../assets/icon-256.png';
  let { size = 64 }: { size?: number } = $props();
</script>

<img src={icon} width={size} height={size} alt="" draggable="false" />

<style>
  img {
    display: block;
    object-fit: contain;
  }
</style>
