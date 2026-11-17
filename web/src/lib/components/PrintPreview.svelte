<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sessionToken } from '$lib/stores/app';
  import { printApi } from '$lib/api/app';
  import { toast } from '$lib/toasts';
  import Modal from '$lib/components/Modal.svelte';
  import Button from '$lib/components/Button.svelte';
  import Alert from '$lib/components/Alert.svelte';
  import { count } from '$lib/format';

  let {
    documentType,
    id,
    title = 'معاينة الطباعة',
    onClose
  }: { documentType: string; id: number; title?: string; onClose: () => void } = $props();

  let loading = $state(true);
  let error = $state('');
  let pdfUrl = $state('');
  let copies = $state(1);
  let printing = $state(false);

  onMount(async () => {
    const token = get(sessionToken);
    if (!token) return;
    try {
      const b64 = await printApi.preview(token, documentType, id);
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      pdfUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
    } catch (e) {
      error = (e as any)?.message ?? 'تعذّر توليد المعاينة';
    } finally {
      loading = false;
    }
  });

  async function execute() {
    const token = get(sessionToken);
    if (!token) return;
    printing = true;
    try {
      const r = await printApi.execute(token, documentType, id, copies);
      toast.ok(`تم تجهيز PDF (${count(r.pages)} صفحة) — ${r.path}`);
    } catch (e) {
      toast.error((e as any)?.message ?? 'تعذّر الطباعة');
    } finally {
      printing = false;
    }
  }
</script>

<Modal open={true} {title} subtitle="يُستخدم اسم المركز والشعار والتذييل من الإعدادات" onClose={onClose} width="940px">
  <div class="ec-toolbar mb-3" style="margin-bottom:0">
    <label class="ec-label" for="copies">عدد النسخ</label>
    <input id="copies" class="ec-input ec-num" style="width:80px" type="number" min="1" max="20" bind:value={copies} dir="ltr" />
    <div class="flex-1"></div>
    <Button variant="primary" size="sm" loading={printing} onclick={execute}>🖨 طباعة / حفظ PDF</Button>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-20"><div class="ec-skel h-96 w-full rounded-xl"></div></div>
  {:else if error}
    <Alert kind="error">{error}</Alert>
  {:else}
    <div class="flex justify-center overflow-auto rounded-xl" style="height: min(62vh, 680px); border: 1px solid var(--ec-border); background: #eef1f6">
      <img src={pdfUrl} alt="معاينة المستند" class="max-w-full" style="box-shadow: 0 4px 20px rgba(0,0,0,.15)" />
    </div>
  {/if}
</Modal>
