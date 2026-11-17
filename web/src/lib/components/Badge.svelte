<script lang="ts">
  import { statusInfo, toneClass, type Tone } from '$lib/format';

  let {
    status = '',
    label = '',
    tone,
    dot = true
  }: {
    status?: string;
    label?: string;
    tone?: Tone;
    dot?: boolean;
  } = $props();

  const info = $derived(
    status ? statusInfo(status) : { label: label || '—', tone: tone ?? ('muted' as Tone) }
  );
  const text = $derived(label || info.label);
  const cls = $derived(tone ? toneClass(tone) : toneClass(info.tone));
</script>

<span class="ec-badge {cls} {dot ? '' : 'no-dot'}">{text}</span>
